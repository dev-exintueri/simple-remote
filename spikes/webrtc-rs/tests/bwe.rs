// Spike only (throwaway).
//! Q5. Does GCC (send-side, TWCC feedback) track a bottleneck: settle near 1 Mbps, then climb
//! when the link opens to 5 Mbps?
//!
//! - Offerer (sender): `configure_congestion_control(Registry::new(), estimator, Twcc, &mut me)`
//!   then `register_default_interceptors`, like examples/bandwidth-estimation-from-disk.
//!   The estimator is `Gcc::new(initial, min, max)` wrapped so every update publishes
//!   `target_bitrate()` to an `AtomicU64` the test can read.
//! - Answerer (receiver): `register_default_interceptors` only; it includes the TWCC receiver
//!   that sends the feedback GCC needs.
//! - Path: forwarder in front of the answerer (SDP rewrite as in external_candidate.rs) with a
//!   rate limit and a 200 ms drop-tail queue on the media direction. Phase 1: 1 Mbps; phase 2:
//!   5 Mbps.
//! - Source: synthetic H.264 at 30 fps whose frame size follows the current target, like an
//!   encoder under rate control (GCC only grows while the sender actually fills the target).
//!
//! Prints a per-second timeline: target, bytes delivered past the bottleneck, drops.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use bytes::Bytes;
use common::*;
use rtc::interceptor::{BandwidthEstimator, EstimatorStats, Gcc, PacketReport};
use rtc::media::Sample;
use rtc::media_stream::MediaStreamTrack;
use rtc::peer_connection::configuration::interceptor_registry::{
    CongestionFeedback, configure_congestion_control, register_default_interceptors,
};
use rtc::peer_connection::configuration::media_engine::MIME_TYPE_H264;
use rtc::rtp_transceiver::rtp_sender::{
    RTCRtpCodec, RTCRtpCodecParameters, RTCRtpCodingParameters, RTCRtpEncodingParameters,
    RtpCodecKind,
};
use webrtc::media_stream::track_local::TrackLocal;
use webrtc::media_stream::track_local::static_sample::TrackLocalStaticSample;
use webrtc::media_stream::track_remote::TrackRemoteEvent;
use webrtc::peer_connection::{MediaEngine, Registry};

const INITIAL_BPS: f64 = 2_500_000.0; // above phase-1 capacity, so the decrease path is exercised
const MIN_BPS: f64 = 100_000.0;
const MAX_BPS: f64 = 10_000_000.0;
const PHASE1: Duration = Duration::from_secs(20);
const PHASE2: Duration = Duration::from_secs(20);
const FPS: u32 = 30;

// ───────────── estimator wrapper (as in the example) ─────────────

struct ReportingEstimator<E: BandwidthEstimator> {
    inner: E,
    target: Arc<AtomicU64>,
}

impl<E: BandwidthEstimator> ReportingEstimator<E> {
    fn new(inner: E) -> (Self, Arc<AtomicU64>) {
        let target = Arc::new(AtomicU64::new(inner.target_bitrate().to_bits()));
        (
            Self {
                inner,
                target: Arc::clone(&target),
            },
            target,
        )
    }
    fn publish(&self) {
        self.target
            .store(self.inner.target_bitrate().to_bits(), Ordering::Relaxed);
    }
}

impl<E: BandwidthEstimator> BandwidthEstimator for ReportingEstimator<E> {
    fn on_reports(&mut self, now: Instant, reports: &[PacketReport]) {
        self.inner.on_reports(now, reports);
        self.publish();
    }
    fn target_bitrate(&self) -> f64 {
        self.inner.target_bitrate()
    }
    fn handle_timeout(&mut self, now: Instant) {
        self.inner.handle_timeout(now);
        self.publish();
    }
    fn poll_timeout(&self) -> Option<Instant> {
        self.inner.poll_timeout()
    }
    fn stats(&self) -> EstimatorStats {
        self.inner.stats()
    }
}

fn h264_codec() -> RTCRtpCodec {
    RTCRtpCodec {
        mime_type: MIME_TYPE_H264.to_owned(),
        clock_rate: 90000,
        channels: 0,
        sdp_fmtp_line: "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f"
            .to_owned(),
        rtcp_feedback: vec![],
    }
}

fn media_engine() -> TestResult<MediaEngine> {
    let mut me = MediaEngine::default();
    me.register_codec(
        RTCRtpCodecParameters {
            rtp_codec: h264_codec(),
            payload_type: 102,
            ..Default::default()
        },
        RtpCodecKind::Video,
    )?;
    Ok(me)
}

fn frame(bytes: usize, seq: u32) -> Bytes {
    // One non-IDR slice NAL; body bytes non-zero so no start code appears inside.
    let len = bytes.max(16);
    let mut v = Vec::with_capacity(len + 4);
    v.extend_from_slice(&[0, 0, 0, 1, 0x41]);
    for i in 0..len {
        v.push(((i as u32 + seq) % 250 + 1) as u8);
    }
    Bytes::from(v)
}

fn load(a: &AtomicU64) -> f64 {
    f64::from_bits(a.load(Ordering::Relaxed))
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

#[tokio::test(flavor = "multi_thread")]
async fn gcc_follows_bottleneck() -> TestResult {
    // Sender with congestion control.
    let mut me = media_engine()?;
    let (estimator, target) = ReportingEstimator::new(Gcc::new(INITIAL_BPS, MIN_BPS, MAX_BPS));
    let registry = configure_congestion_control(
        Registry::new(),
        estimator,
        CongestionFeedback::Twcc,
        &mut me,
    )?;
    let registry = register_default_interceptors(registry, &mut me)?;
    let (offerer, mut oe) = build_peer(
        "offerer",
        "127.0.0.1:0",
        Some((me, registry)),
        setting_engine().build(),
    )
    .await?;

    // Receiver.
    let mut me = media_engine()?;
    let registry = register_default_interceptors(Registry::new(), &mut me)?;
    let (answerer, mut ae) = build_peer(
        "answerer",
        "127.0.0.1:0",
        Some((me, registry)),
        setting_engine().build(),
    )
    .await?;

    let ssrc: u32 = 0x0BAD_CAFE;
    let track = Arc::new(TrackLocalStaticSample::new(
        Instant::now(),
        MediaStreamTrack::new(
            "q5-stream".to_owned(),
            "q5-video".to_owned(),
            "q5-label".to_owned(),
            RtpCodecKind::Video,
            vec![RTCRtpEncodingParameters {
                rtp_coding_parameters: RTCRtpCodingParameters {
                    ssrc: Some(ssrc),
                    ..Default::default()
                },
                codec: h264_codec(),
                ..Default::default()
            }],
        ),
    )?);
    let sender = offerer
        .add_track(Arc::clone(&track) as Arc<dyn TrackLocal>)
        .await?;

    // Signaling through the forwarder.
    let offer = make_offer(&offerer, &mut oe, None).await?;
    let offer = with_sdp(&offer, strip_candidates(&offer.sdp))?;
    let answer = make_answer(&answerer, &mut ae, offer).await?;
    let target_addr = *host_candidates(&answer.sdp)
        .first()
        .ok_or("answerer has no host candidate")?;
    let fwd = Forwarder::start("127.0.0.2:0", target_addr).await?;
    fwd.rate_bps.store(1_000_000, Ordering::Relaxed);
    let srflx = srflx_candidate_line(fwd.public_addr, target_addr);
    let answer = with_sdp(&answer, replace_candidates(&answer.sdp, &[srflx]))?;
    assert!(
        answer.sdp.contains("transport-cc"),
        "answer negotiates TWCC feedback"
    );
    offerer.set_remote_description(answer).await?;
    wait_connected(&mut oe, CONNECT_TIMEOUT).await?;
    wait_connected(&mut ae, CONNECT_TIMEOUT).await?;

    let pt = sender
        .get_parameters()
        .await?
        .rtp_parameters
        .codecs
        .first()
        .map(|c| c.payload_type)
        .ok_or("sender has no negotiated codec")?;

    // Receiver: drain RTP so the track queue never backs up; count payload bytes.
    let rx_bytes = Arc::new(AtomicU64::new(0));
    {
        let rx_bytes = Arc::clone(&rx_bytes);
        let mut tracks_rx =
            std::mem::replace(&mut ae.tracks, tokio::sync::mpsc::unbounded_channel().1);
        tokio::spawn(async move {
            while let Some(remote) = tracks_rx.recv().await {
                let rx_bytes = Arc::clone(&rx_bytes);
                tokio::spawn(async move {
                    while let Some(ev) = remote.poll().await {
                        match ev {
                            TrackRemoteEvent::OnRtpPacket(p) => {
                                rx_bytes.fetch_add(p.payload.len() as u64, Ordering::Relaxed);
                            }
                            TrackRemoteEvent::OnEnded => break,
                            _ => {}
                        }
                    }
                });
            }
        });
    }

    // Sender: frame size follows the current target.
    let running = Arc::new(AtomicBool::new(true));
    let tx_bytes = Arc::new(AtomicU64::new(0));
    let send_task = {
        let (track, target, running, tx_bytes) = (
            Arc::clone(&track),
            Arc::clone(&target),
            Arc::clone(&running),
            Arc::clone(&tx_bytes),
        );
        tokio::spawn(async move {
            let period = Duration::from_secs(1) / FPS;
            let mut tick = tokio::time::interval(period);
            let mut seq = 0u32;
            while running.load(Ordering::Relaxed) {
                tick.tick().await;
                let bytes = (load(&target) / 8.0 / FPS as f64) as usize;
                let sample = Sample {
                    data: frame(bytes, seq),
                    duration: period,
                    ..Sample::new(Instant::now())
                };
                if track.write_sample(ssrc, pt, &sample, &[]).await.is_err() {
                    break;
                }
                tx_bytes.fetch_add(bytes as u64, Ordering::Relaxed);
                seq = seq.wrapping_add(1);
            }
        })
    };

    // Timeline.
    let t0 = Instant::now();
    let mut phase1 = Vec::new();
    let mut phase2 = Vec::new();
    let (mut last_rx, mut last_tx, mut last_drop) = (0u64, 0u64, 0u64);
    println!("  t   cap(Mbps)  target(Mbps)  sent(Mbps)  delivered(Mbps)  drops");
    let total = (PHASE1 + PHASE2).as_secs();
    for s in 1..=total {
        if s == PHASE1.as_secs() + 1 {
            fwd.rate_bps.store(5_000_000, Ordering::Relaxed);
        }
        tokio::time::sleep_until(tokio::time::Instant::from_std(t0 + Duration::from_secs(s))).await;
        let rx = rx_bytes.load(Ordering::Relaxed);
        let tx = tx_bytes.load(Ordering::Relaxed);
        let dr = fwd.inbound_dropped.load(Ordering::Relaxed);
        let tgt = load(&target);
        let cap = fwd.rate_bps.load(Ordering::Relaxed) as f64;
        println!(
            "{s:>3}   {:>8.2}   {:>10.2}   {:>9.2}   {:>14.2}   {:>5}",
            cap / 1e6,
            tgt / 1e6,
            (tx - last_tx) as f64 * 8.0 / 1e6,
            (rx - last_rx) as f64 * 8.0 / 1e6,
            dr - last_drop
        );
        (last_rx, last_tx, last_drop) = (rx, tx, dr);
        if s <= PHASE1.as_secs() {
            phase1.push(tgt);
        } else {
            phase2.push(tgt);
        }
    }
    running.store(false, Ordering::Relaxed);
    let _ = send_task.await;

    // Loose assertions: settle under ~1.2 Mbps with a 1 Mbps link (median of the last 5 s),
    // then rise clearly once the link is 5 Mbps.
    let p1_tail = median(phase1[phase1.len() - 5..].to_vec());
    let p2_max = phase2.iter().cloned().fold(0.0, f64::max);
    let p2_tail = median(phase2[phase2.len() - 5..].to_vec());
    println!(
        "phase1 tail median {:.2} Mbps; phase2 max {:.2} Mbps, tail median {:.2} Mbps",
        p1_tail / 1e6,
        p2_max / 1e6,
        p2_tail / 1e6
    );
    assert!(p1_tail < 1_200_000.0, "phase 1 settles below ~1.2 Mbps");
    assert!(
        p2_max > p1_tail * 1.5,
        "phase 2 estimate increases once the bottleneck opens"
    );

    offerer.close().await?;
    answerer.close().await?;
    Ok(())
}
