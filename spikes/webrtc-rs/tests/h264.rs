// Spike only (throwaway).
//! Q3. H.264 over TrackLocalStaticSample: do large NAL units survive FU-A fragmentation and
//! reassembly byte for byte, and does a PLI from the viewer reach the sending application?
//!
//! Sender: synthetic Annex-B access units (SPS + PPS + IDR, then P slices). IDR and P slices are
//! 5000 bytes, so at the 1200-byte packetizer MTU each becomes ~5 FU-A packets.
//! SPS/PPS stay small on purpose: `H264Payloader` does not send SPS/PPS on their own; it holds
//! them and emits one STAP-A (SPS+PPS) in front of the next NAL, and only `if stap_a.len() <= mtu`
//! (rtc-rtp h264/mod.rs:104). An oversized SPS/PPS would be dropped silently; the unit test at
//! the bottom pins that behaviour.
//!
//! Receiver: `on_track` → `TrackRemote::poll` → `OnRtpPacket` → `H264Packet::depacketize`
//! (Annex-B output, 4-byte start codes) → split into NAL units → compare with what was sent.
//!
//! PLI: the answerer calls `TrackRemote::write_rtcp(PictureLossIndication)`. RTCP reaching the
//! offerer is consumed by the interceptor chain unless something marks it
//! `Attribute::DeliverToApplication`; a custom interceptor (copied in spirit from
//! examples/rtcp-processing) does that for PLI/FIR, and the offerer reads it from
//! `TrackLocal::poll` as `TrackLocalEvent::OnRtcpPacket`.

mod common;

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use bytes::Bytes;
use common::*;
use rtc::interceptor::{Attribute, Interceptor, Packet, Slot, StreamInfo, TaggedPacket};
use rtc::media::Sample;
use rtc::media_stream::MediaStreamTrack;
use rtc::peer_connection::configuration::interceptor_registry::register_default_interceptors;
use rtc::peer_connection::configuration::media_engine::MIME_TYPE_H264;
use rtc::rtcp::payload_feedbacks::full_intra_request::FullIntraRequest;
use rtc::rtcp::payload_feedbacks::picture_loss_indication::PictureLossIndication;
use rtc::rtp::codec::h264::{H264Packet, H264Payloader};
use rtc::rtp::packetizer::{Depacketizer, Payloader};
use rtc::rtp_transceiver::rtp_sender::{
    RTCRtpCodec, RTCRtpCodecParameters, RTCRtpCodingParameters, RTCRtpEncodingParameters,
    RtpCodecKind,
};
use rtc::sansio::Protocol;
use rtc::shared::error::Error;
use tokio::sync::mpsc;
use webrtc::media_stream::Track;
use webrtc::media_stream::track_local::static_sample::TrackLocalStaticSample;
use webrtc::media_stream::track_local::{TrackLocal, TrackLocalEvent};
use webrtc::media_stream::track_remote::TrackRemoteEvent;
use webrtc::peer_connection::{MediaEngine, Registry};

const FRAME: Duration = Duration::from_millis(33);
const BIG: usize = 5000;
const P_FRAMES: usize = 5;

// ───────────── interceptor: surface PLI/FIR to the application ─────────────

#[derive(Default)]
struct KeyframeRequestToApp {
    read_queue: VecDeque<TaggedPacket>,
    write_queue: VecDeque<TaggedPacket>,
}

fn is_keyframe_request(p: &Box<dyn rtc::rtcp::Packet>) -> bool {
    let any = p.as_any();
    any.is::<PictureLossIndication>() || any.is::<FullIntraRequest>()
}

impl Protocol<TaggedPacket, TaggedPacket, ()> for KeyframeRequestToApp {
    type Rout = TaggedPacket;
    type Wout = TaggedPacket;
    type Eout = ();
    type Error = Error;
    type Time = Instant;

    fn handle_read(&mut self, mut msg: TaggedPacket) -> Result<(), Self::Error> {
        if let Packet::Rtcp(packets) = &msg.message.packet {
            let requests: Vec<Box<dyn rtc::rtcp::Packet>> = packets
                .iter()
                .filter(|p| is_keyframe_request(p))
                .cloned()
                .collect();
            if !requests.is_empty() {
                msg.message.packet = Packet::Rtcp(requests);
                msg.message.add(Attribute::DeliverToApplication);
            }
        }
        self.read_queue.push_back(msg);
        Ok(())
    }
    fn poll_read(&mut self) -> Option<Self::Rout> {
        self.read_queue.pop_front()
    }
    fn handle_write(&mut self, msg: TaggedPacket) -> Result<(), Self::Error> {
        self.write_queue.push_back(msg);
        Ok(())
    }
    fn poll_write(&mut self) -> Option<Self::Wout> {
        self.write_queue.pop_front()
    }
}

impl Interceptor for KeyframeRequestToApp {
    fn bind_local_stream(&mut self, _info: &StreamInfo) {}
    fn unbind_local_stream(&mut self, _info: &StreamInfo) {}
    fn bind_remote_stream(&mut self, _info: &StreamInfo) {}
    fn unbind_remote_stream(&mut self, _info: &StreamInfo) {}
}

// ───────────── H.264 helpers ─────────────

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

fn media(with_pli_to_app: bool) -> TestResult<(MediaEngine, Registry)> {
    let mut me = MediaEngine::default();
    me.register_codec(
        RTCRtpCodecParameters {
            rtp_codec: h264_codec(),
            payload_type: 102,
            ..Default::default()
        },
        RtpCodecKind::Video,
    )?;
    let mut registry = register_default_interceptors(Registry::new(), &mut me)?;
    if with_pli_to_app {
        // Past JitterBuffer (13_000): read walks the chain forwards, so this is the last
        // interceptor an incoming packet meets before the application.
        registry = registry.with(Slot::from(14_000), KeyframeRequestToApp::default());
    }
    Ok((me, registry))
}

/// NAL unit = header byte + body. Body bytes are 1..=250 so no start code can appear inside.
fn nal(header: u8, len: usize, seed: u8) -> Vec<u8> {
    let mut v = Vec::with_capacity(len);
    v.push(header);
    for i in 1..len {
        v.push(((i + seed as usize) % 250 + 1) as u8);
    }
    v
}

fn annex_b(nals: &[&[u8]]) -> Bytes {
    let mut out = Vec::new();
    for n in nals {
        out.extend_from_slice(&[0, 0, 0, 1]);
        out.extend_from_slice(n);
    }
    Bytes::from(out)
}

/// Splits an Annex-B stream on 3- or 4-byte start codes.
fn split_annex_b(data: &[u8]) -> Vec<Vec<u8>> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 <= data.len() {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            starts.push(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }
    let mut out = Vec::new();
    for (k, &s) in starts.iter().enumerate() {
        let mut e = if k + 1 < starts.len() {
            starts[k + 1] - 3
        } else {
            data.len()
        };
        while e > s && data[e - 1] == 0 {
            e -= 1; // leading zero of a 4-byte start code belongs to no NAL
        }
        out.push(data[s..e].to_vec());
    }
    out
}

// ───────────── test ─────────────

#[tokio::test(flavor = "multi_thread")]
async fn h264_fua_roundtrip_and_pli_to_sender_app() -> TestResult {
    let (offerer, mut oe) = build_peer(
        "offerer",
        "127.0.0.1:0",
        Some(media(true)?),
        setting_engine().build(),
    )
    .await?;
    let (answerer, mut ae) = build_peer(
        "answerer",
        "127.0.0.1:0",
        Some(media(false)?),
        setting_engine().build(),
    )
    .await?;

    let ssrc: u32 = 0x1234_5678;
    let track = Arc::new(TrackLocalStaticSample::new(
        Instant::now(),
        MediaStreamTrack::new(
            "q3-stream".to_owned(),
            "q3-video".to_owned(),
            "q3-label".to_owned(),
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

    negotiate(&offerer, &mut oe, &answerer, &mut ae, None).await?;
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
    assert_eq!(track.ssrcs().await, vec![ssrc]);
    println!("negotiated payload type {pt}");

    // Offerer: count keyframe requests surfaced through TrackLocal::poll.
    let pli_seen = Arc::new(AtomicU32::new(0));
    {
        let pli_seen = Arc::clone(&pli_seen);
        let t = Arc::clone(&track);
        tokio::spawn(async move {
            while let Some(ev) = t.poll().await {
                if let TrackLocalEvent::OnRtcpPacket(pkts) = ev {
                    let n = pkts
                        .iter()
                        .filter(|p| p.as_any().is::<PictureLossIndication>())
                        .count();
                    pli_seen.fetch_add(n as u32, Ordering::SeqCst);
                }
            }
        });
    }

    // Warm-up: the TrackRemote is created on the first RTP packet, and events and media travel
    // in separate queues, so the earliest packets are not guaranteed to reach poll(). Send small
    // single-NAL P slices until the answerer has its track, then send the real sequence.
    let warm = nal(0x41, 20, 7);
    let (nal_tx, mut nal_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let remote = loop {
        track
            .write_sample(
                ssrc,
                pt,
                &Sample {
                    data: annex_b(&[&warm]),
                    duration: FRAME,
                    ..Sample::new(Instant::now())
                },
                &[],
            )
            .await?;
        if let Ok(Some(t)) = tokio::time::timeout(FRAME, ae.tracks.recv()).await {
            break t;
        }
    };
    println!("answerer on_track, ssrcs {:?}", remote.ssrcs().await);

    // Answerer: depacketize everything into NAL units.
    {
        let remote = Arc::clone(&remote);
        tokio::spawn(async move {
            let mut depack = H264Packet::default();
            while let Some(ev) = remote.poll().await {
                match ev {
                    TrackRemoteEvent::OnRtpPacket(pkt) => match depack.depacketize(&pkt.payload) {
                        Ok(out) if !out.is_empty() => {
                            for n in split_annex_b(&out) {
                                let _ = nal_tx.send(n);
                            }
                        }
                        Ok(_) => {} // middle of an FU-A
                        Err(e) => eprintln!("depacketize error: {e}"),
                    },
                    TrackRemoteEvent::OnEnded => break,
                    _ => {}
                }
            }
        });
    }
    // Answerer: PLI every 200 ms.
    {
        let remote = Arc::clone(&remote);
        let media_ssrc = remote.ssrcs().await.first().copied().unwrap_or(ssrc);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_millis(200));
            loop {
                tick.tick().await;
                let pli = PictureLossIndication {
                    sender_ssrc: 0,
                    media_ssrc,
                };
                if remote.write_rtcp(vec![Box::new(pli)]).await.is_err() {
                    break;
                }
            }
        });
    }

    // The real sequence: AU0 = SPS+PPS+IDR, then P slices, one AU per frame.
    let sps = nal(0x67, 12, 1);
    let pps = nal(0x68, 5, 2);
    let idr = nal(0x65, BIG, 3);
    let ps: Vec<Vec<u8>> = (0..P_FRAMES)
        .map(|k| nal(0x41, BIG, 10 + k as u8))
        .collect();

    let mut expected: Vec<Vec<u8>> = vec![sps.clone(), pps.clone(), idr.clone()];
    expected.extend(ps.iter().cloned());

    let mut aus: Vec<Bytes> = vec![annex_b(&[&sps, &pps, &idr])];
    aus.extend(ps.iter().map(|p| annex_b(&[p])));
    for au in aus {
        track
            .write_sample(
                ssrc,
                pt,
                &Sample {
                    data: au,
                    duration: FRAME,
                    ..Sample::new(Instant::now())
                },
                &[],
            )
            .await?;
        tokio::time::sleep(FRAME).await;
    }

    // Collect: skip warm-up NALs until the SPS, then take expected.len() NALs.
    let mut got: Vec<Vec<u8>> = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while got.len() < expected.len() {
        let n = tokio::time::timeout_at(deadline, nal_rx.recv())
            .await
            .map_err(|_| format!("only {} of {} NALs arrived", got.len(), expected.len()))?
            .ok_or("receiver ended")?;
        if got.is_empty() && n.first() != Some(&0x67) {
            continue;
        }
        got.push(n);
    }
    for (i, (g, e)) in got.iter().zip(expected.iter()).enumerate() {
        println!(
            "NAL {i}: type {} len {} (sent {})",
            g[0] & 0x1f,
            g.len(),
            e.len()
        );
    }
    assert_eq!(got, expected, "reassembled NAL units equal what was sent");

    // PLI reached the sender's application.
    let deadline = Instant::now() + Duration::from_secs(5);
    while pli_seen.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let plis = pli_seen.load(Ordering::SeqCst);
    println!("PLIs seen by offerer app: {plis}");
    assert!(plis >= 1, "PLI surfaced as TrackLocalEvent::OnRtcpPacket");

    offerer.close().await?;
    answerer.close().await?;
    Ok(())
}

/// Pins the payloader behaviour described at the top: SPS/PPS are only ever sent as a STAP-A in
/// front of the next NAL, and that STAP-A is dropped if it exceeds the MTU.
#[test]
fn payloader_drops_oversized_sps_pps_stap_a() -> TestResult {
    let mtu = 1200;
    let small = annex_b(&[&nal(0x67, 12, 1), &nal(0x68, 5, 2), &nal(0x65, 100, 3)]);
    let out = H264Payloader::default().payload(mtu, &small)?;
    assert_eq!(out.len(), 2, "STAP-A(SPS,PPS) + IDR");
    assert_eq!(out[0][0] & 0x1f, 24, "first payload is STAP-A");

    let big = annex_b(&[&nal(0x67, BIG, 1), &nal(0x68, 5, 2), &nal(0x65, 100, 3)]);
    let out = H264Payloader::default().payload(mtu, &big)?;
    assert_eq!(
        out.len(),
        1,
        "oversized SPS+PPS STAP-A silently dropped, only IDR left"
    );
    assert_eq!(out[0][0] & 0x1f, 5);
    Ok(())
}
