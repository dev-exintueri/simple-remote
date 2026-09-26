// Spike only (throwaway).
//! Q5. Does str0m's send-side BWE (TWCC) track a bottleneck and recover when it opens?
//!
//! Setup: L sends H.264 at a rate that follows the latest estimate (like a real encoder
//! would), with `set_desired_bitrate(5 Mbps)` so str0m probes/pads above the media rate.
//! Phase 1 (20 s): L->R bottleneck 1 Mbps, 20 ms latency, 50 KB queue (~400 ms).
//! Phase 2 (20 s): bottleneck raised to 5 Mbps.
//!
//! Source facts (str0m 0.23.1):
//! - `RtcConfig::enable_bwe(Option<Bitrate>)` in config.rs (Some = on, initial estimate).
//! - `Bitrate::{bps,kbps,mbps}` const fns, `as_u64`, `Display` in str0m-proto bandwidth.rs;
//!   re-exported as `str0m::bwe::Bitrate`.
//! - `rtc.bwe().set_desired_bitrate(..)` in bwe/api.rs; estimates arrive as
//!   `Event::EgressBitrateEstimate(BweKind::Twcc(Bitrate))`.
//! - TWCC feedback is enabled automatically in SDP mode when an m-line has
//!   `a=rtcp-fb:* transport-cc` and the transport-cc header extension (change/sdp.rs,
//!   `session.enable_twcc_feedback()`); upstream tests/bwe/common.rs needs the explicit
//!   call only because it uses the direct API.
//! - Pacer runs at 2x the estimate (session.rs `PacerImpl::leaky_bucket(rate * 2.0)`),
//!   padding/probing needs a padding queue (RTX), which SDP negotiation sets up.
mod common;

use std::time::{Duration, Instant};

use common::{Sim, addr, both_connected, negotiate};
use str0m::bwe::{Bitrate, BweKind};
use str0m::format::Codec;
use str0m::media::{Direction, MediaKind, MediaTime, Mid, Pt};
use str0m::{Candidate, Event, Rtc, RtcError};

const FPS: u64 = 30;

fn last_estimate(sim: &Sim) -> Option<(Duration, Bitrate)> {
    sim.l.events.iter().rev().find_map(|(t, e)| match e {
        Event::EgressBitrateEstimate(BweKind::Twcc(b)) => Some((*t, *b)),
        _ => None,
    })
}

fn estimates_between(sim: &Sim, from: Duration, to: Duration) -> Vec<(Duration, Bitrate)> {
    sim.l
        .events
        .iter()
        .filter_map(|(t, e)| match e {
            Event::EgressBitrateEstimate(BweKind::Twcc(b)) if *t >= from && *t < to => {
                Some((*t, *b))
            }
            _ => None,
        })
        .collect()
}

fn delta_frame(len: usize, seed: u64) -> Vec<u8> {
    let mut v = Vec::with_capacity(5 + len);
    v.extend_from_slice(&[0, 0, 0, 1, 0x41]);
    v.extend((0..len).map(|i| ((i as u64 + seed) % 250 + 1) as u8));
    v
}

struct Sender {
    mid: Mid,
    pt: Pt,
    frame_no: u64,
    sent_bytes: u64,
}

impl Sender {
    fn run_phase(&mut self, sim: &mut Sim, dur: Duration, label: &str) -> Result<(), RtcError> {
        let end = sim.now + dur;
        let mut next_print = sim.now;
        while sim.now < end {
            let target = last_estimate(sim)
                .map(|(_, b)| b)
                .unwrap_or(Bitrate::kbps(300))
                .clamp(Bitrate::kbps(200), Bitrate::mbps(4));
            let frame_len = (target.as_u64() / 8 / FPS) as usize;
            let frame = delta_frame(frame_len.max(200), self.frame_no);
            self.sent_bytes += frame.len() as u64;
            let wallclock: Instant = sim.now;
            sim.l.rtc.writer(self.mid).expect("writer").write(
                self.pt,
                wallclock,
                MediaTime::from_90khz(self.frame_no * (90_000 / FPS)),
                frame,
            )?;
            self.frame_no += 1;
            sim.run_for(Duration::from_micros(1_000_000 / FPS))?;

            if sim.now >= next_print {
                let est = last_estimate(sim).map(|(_, b)| b.to_string());
                println!(
                    "TIMELINE {label} t={:>6.2}s est={:<12} media_target={} qdelay={:>4}ms l_to_r={:?}",
                    sim.elapsed().as_secs_f64(),
                    est.unwrap_or_else(|| "-".into()),
                    target,
                    sim.l_to_r.queue_delay(sim.now).as_millis(),
                    sim.l_to_r.stats
                );
                next_print += Duration::from_secs(1);
            }
        }
        Ok(())
    }
}

#[test]
fn bwe_tracks_bottleneck_then_recovers() -> Result<(), RtcError> {
    let now = Instant::now();
    let l_rtc = Rtc::builder().enable_bwe(Some(Bitrate::kbps(300))).build(now);
    let r_rtc = Rtc::builder().build(now);
    let mut sim = Sim::new(now, l_rtc, r_rtc);
    sim.l.verbose = true;
    sim.r.verbose = false;
    sim.l_to_r.latency = Duration::from_millis(20);
    sim.r_to_l.latency = Duration::from_millis(20);
    sim.l
        .rtc
        .add_local_candidate(Candidate::host(addr("127.0.0.1:10001"), "udp").unwrap())
        .unwrap();
    sim.r
        .rtc
        .add_local_candidate(Candidate::host(addr("127.0.0.1:10002"), "udp").unwrap())
        .unwrap();

    let (mid, offer_sdp, _) = negotiate(&mut sim.l.rtc, &mut sim.r.rtc, |api| {
        api.add_media(MediaKind::Video, Direction::SendOnly, None, None, None)
    })?;
    assert!(offer_sdp.contains("transport-cc"), "TWCC negotiated in SDP");

    sim.run_until(Duration::from_secs(10), Duration::from_millis(5), |s| both_connected(s))?
        .expect("connect");

    let pt = sim
        .l
        .rtc
        .codec_config()
        .find(|p| p.spec().codec == Codec::H264 && p.spec().format.packetization_mode == Some(1))
        .map(|p| p.pt())
        .expect("H264 pt");
    sim.l.rtc.bwe().set_desired_bitrate(Bitrate::mbps(5));

    // Phase 1: 1 Mbps bottleneck.
    sim.l_to_r.rate_bps = Some(1_000_000);
    sim.l_to_r.queue_limit_bytes = 50_000;
    let mut tx = Sender {
        mid,
        pt,
        frame_no: 0,
        sent_bytes: 0,
    };
    let p1_start = sim.elapsed();
    tx.run_phase(&mut sim, Duration::from_secs(20), "1Mbps")?;
    let p1_end = sim.elapsed();

    // Phase 2: 5 Mbps.
    sim.l_to_r.rate_bps = Some(5_000_000);
    sim.l_to_r.queue_limit_bytes = 250_000;
    tx.run_phase(&mut sim, Duration::from_secs(20), "5Mbps")?;
    let p2_end = sim.elapsed();

    let p1_tail = estimates_between(&sim, p1_end - Duration::from_secs(5), p1_end);
    let p1_all = estimates_between(&sim, p1_start, p1_end);
    let p2_all = estimates_between(&sim, p1_end, p2_end);
    let p1_last = p1_all.last().map(|(_, b)| *b).expect("estimates in phase 1");
    let p1_tail_max = p1_tail.iter().map(|(_, b)| b.as_u64()).max();
    let p1_peak = p1_all.iter().map(|(_, b)| b.as_u64()).max().unwrap();
    let p2_max = p2_all.iter().map(|(_, b)| b.as_u64()).max().expect("estimates in phase 2");
    let p2_last = p2_all.last().map(|(_, b)| *b).unwrap();

    println!(
        "MEASURE phase1: n_est={} peak={} last={} tail5s_max={:?}",
        p1_all.len(),
        Bitrate::bps(p1_peak),
        p1_last,
        p1_tail_max.map(Bitrate::bps)
    );
    println!(
        "MEASURE phase2: n_est={} max={} last={} first_above_1.5M_at={:?}",
        p2_all.len(),
        Bitrate::bps(p2_max),
        p2_last,
        p2_all
            .iter()
            .find(|(_, b)| b.as_u64() > 1_500_000)
            .map(|(t, _)| *t - p1_end)
    );
    println!(
        "MEASURE media_bytes_written={} l_to_r={:?}",
        tx.sent_bytes, sim.l_to_r.stats
    );

    assert!(
        p1_last.as_u64() <= 1_200_000,
        "phase 1 estimate should settle at or below ~1.2 Mbps, got {p1_last}"
    );
    assert!(
        p2_max > 1_500_000,
        "phase 2 estimate should rise above 1.5 Mbps once capacity is 5 Mbps, max {}",
        Bitrate::bps(p2_max)
    );
    Ok(())
}
