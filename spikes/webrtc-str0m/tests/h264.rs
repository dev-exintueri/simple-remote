// Spike only (throwaway).
//! Q3. H.264 over str0m's frame-level API, and keyframe requests.
//!
//! Source facts (str0m 0.23.1):
//! - packet/h264.rs `H264Packetizer::packetize`: splits Annex-B input on 3/4-byte start
//!   codes. SPS/PPS are held and sent as one STAP-A before the next NAL. NALs larger
//!   than the MTU become FU-A fragments.
//! - packet/h264.rs `H264Depacketizer::depacketize` (is_avc=false, the default): every
//!   NAL (single, from STAP-A, or reassembled FU-A with header `ref_idc|type`) is written
//!   as `00 00 00 01` + NAL. So input written with 4-byte start codes and NAL header
//!   forbidden bit 0 comes out byte-identical in `MediaData::data`.
//! - Keyframe request direction: the RECEIVER calls `rtc.writer(mid)` and then
//!   `Writer::request_keyframe(rid, KeyframeRequestKind::Pli)` (media/writer.rs). It looks
//!   up the *receive* stream (`stream_rx_by_midrid`) and fails with NoReceiverSource if
//!   no RTP has arrived yet. The SENDER gets `Event::KeyframeRequest` (lib.rs Event).
//!   Same pattern in upstream tests/keyframe-requests.rs.
//! - Payload bytes must not contain `00 00 01`, otherwise the packetizer would split
//!   there. Dummy payload bytes below are never 0.
mod common;

use std::time::Duration;

use common::{Sim, addr, both_connected, negotiate};
use str0m::format::{Codec, CodecExtra};
use str0m::media::{Direction, KeyframeRequestKind, MediaKind, MediaTime};
use str0m::{Candidate, Event, Rtc, RtcError};

fn nal(header: u8, len: usize, seed: u8) -> Vec<u8> {
    let mut v = Vec::with_capacity(4 + len);
    v.extend_from_slice(&[0, 0, 0, 1, header]);
    v.extend((0..len).map(|i| ((i as u32 + seed as u32) % 250 + 1) as u8));
    v
}

fn frames() -> Vec<Vec<u8>> {
    let mut out = vec![];
    // Keyframe: SPS (7), PPS (8), IDR (5) with 5000 byte payload -> FU-A.
    let mut idr = nal(0x67, 20, 1);
    idr.extend(nal(0x68, 4, 2));
    idr.extend(nal(0x65, 5000, 3));
    out.push(idr);
    // Delta frames: non-IDR slice (1), 3000 bytes -> FU-A.
    for i in 0..5u8 {
        out.push(nal(0x41, 3000, 10 + i));
    }
    out
}

#[test]
fn h264_frames_roundtrip_and_keyframe_request() -> Result<(), RtcError> {
    let now = std::time::Instant::now();
    let mut sim = Sim::new(now, Rtc::builder().build(now), Rtc::builder().build(now));
    sim.l_to_r.latency = Duration::from_millis(10);
    sim.r_to_l.latency = Duration::from_millis(10);
    sim.l
        .rtc
        .add_local_candidate(Candidate::host(addr("127.0.0.1:10001"), "udp").unwrap())
        .unwrap();
    sim.r
        .rtc
        .add_local_candidate(Candidate::host(addr("127.0.0.1:10002"), "udp").unwrap())
        .unwrap();

    let (mid, _, answer_sdp) = negotiate(&mut sim.l.rtc, &mut sim.r.rtc, |api| {
        api.add_media(MediaKind::Video, Direction::SendOnly, None, None, None)
    })?;
    println!("--- answer ---\n{answer_sdp}---");

    sim.run_until(Duration::from_secs(10), Duration::from_millis(5), |s| both_connected(s))?
        .expect("connect");

    let params = sim
        .l
        .rtc
        .codec_config()
        .find(|p| p.spec().codec == Codec::H264 && p.spec().format.packetization_mode == Some(1))
        .cloned()
        .expect("H264 packetization-mode=1 in codec config");
    let pt = params.pt();
    println!("OBSERVE H264 pt={pt:?} spec={:?}", params.spec());

    let sent = frames();
    for (i, f) in sent.iter().enumerate() {
        let wallclock = sim.now;
        let rtp_time = MediaTime::from_90khz(i as u64 * 3000);
        sim.l
            .rtc
            .writer(mid)
            .expect("L writer")
            .write(pt, wallclock, rtp_time, f.clone())?;
        sim.run_for(Duration::from_millis(33))?;
    }
    sim.run_for(Duration::from_millis(500))?;

    let received: Vec<_> = sim
        .r
        .events
        .iter()
        .filter_map(|(t, e)| match e {
            Event::MediaData(d) if d.mid == mid => Some((*t, d)),
            _ => None,
        })
        .collect();
    println!("MEASURE frames_sent={} frames_received={}", sent.len(), received.len());
    assert_eq!(received.len(), sent.len(), "one MediaData per written frame");
    for (i, ((t, d), f)) in received.iter().zip(sent.iter()).enumerate() {
        let key = matches!(d.codec_extra, CodecExtra::H264(e) if e.is_keyframe);
        println!(
            "OBSERVE frame {i} at {t:?}: len={} sent_len={} keyframe={key} contiguous={} seq={:?}",
            d.data.len(),
            f.len(),
            d.contiguous,
            d.seq_range
        );
        assert_eq!(&d.data[..], &f[..], "frame {i} byte-identical after FU-A/STAP-A");
        assert_eq!(key, i == 0, "only frame 0 is a keyframe");
    }
    // 5000 byte IDR must have been fragmented into several RTP packets.
    let first = &received[0].1;
    let n_pkts = (**first.seq_range.end() - **first.seq_range.start()) + 1;
    println!("MEASURE keyframe_rtp_packets={n_pkts}");
    assert!(n_pkts >= 3, "IDR spans multiple RTP packets (STAP-A + FU-A)");

    // --- keyframe request: receiver asks, sender is told ---
    let t_req = sim.elapsed();
    {
        let mut w = sim.r.rtc.writer(mid).expect("R writer (used for requests on recv side)");
        assert!(w.is_request_keyframe_possible(KeyframeRequestKind::Pli));
        w.request_keyframe(None, KeyframeRequestKind::Pli)?;
    }
    let dt = sim
        .run_until(Duration::from_secs(2), Duration::from_millis(1), |s| {
            s.l.has_event(|e| {
                matches!(e, Event::KeyframeRequest(k)
                    if k.mid == mid && k.kind == KeyframeRequestKind::Pli)
            })
        })?
        .expect("sender (L) receives Event::KeyframeRequest");
    println!("MEASURE keyframe_request_at={t_req:?} delivered_after={dt:?}");
    assert!(
        !sim.r.has_event(|e| matches!(e, Event::KeyframeRequest(_))),
        "receiver does not get its own request"
    );
    Ok(())
}
