// Spike only (throwaway).
//! Q4. Under 20 % packet loss, does a reliable ordered channel deliver everything in order,
//! and does an unordered `max_retransmits: Some(0)` channel deliver some but not all?
//!
//! Loss: a forwarder in front of the answerer (same SDP rewrite as external_candidate.rs:
//! offerer candidates stripped, answerer's host candidate replaced by the forwarder address),
//! dropping every 5th datagram offerer → answerer once both channels are open.
//! Messages are 1000 bytes so SCTP cannot bundle several into one datagram; each loss then
//! costs exactly one message on the unreliable channel.

mod common;

use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use bytes::BytesMut;
use common::*;
use webrtc::data_channel::RTCDataChannelInit;

const N: u32 = 200;
const MSG_LEN: usize = 1000;

fn message(i: u32) -> BytesMut {
    let mut b = BytesMut::zeroed(MSG_LEN);
    b[..4].copy_from_slice(&i.to_be_bytes());
    b
}

fn index(m: &[u8]) -> u32 {
    u32::from_be_bytes(m[..4].try_into().unwrap())
}

#[tokio::test(flavor = "multi_thread")]
async fn reliable_vs_unreliable_under_loss() -> TestResult {
    let (offerer, mut oe) =
        build_peer("offerer", "127.0.0.1:0", None, setting_engine().build()).await?;
    let (answerer, mut ae) =
        build_peer("answerer", "127.0.0.1:0", None, setting_engine().build()).await?;

    let reliable = offerer.create_data_channel("reliable", None).await?;
    let unreliable = offerer
        .create_data_channel(
            "unreliable",
            Some(RTCDataChannelInit {
                ordered: false,
                max_retransmits: Some(0),
                ..Default::default()
            }),
        )
        .await?;
    let mut rel_off = spawn_dc_reader(reliable.clone());
    let mut unrel_off = spawn_dc_reader(unreliable.clone());

    // Signaling with the path forced through the forwarder.
    let offer = make_offer(&offerer, &mut oe, None).await?;
    let offer = with_sdp(&offer, strip_candidates(&offer.sdp))?;
    let answer = make_answer(&answerer, &mut ae, offer).await?;
    let target = *host_candidates(&answer.sdp)
        .first()
        .ok_or("answerer has no host candidate")?;
    let fwd = Forwarder::start("127.0.0.2:0", target).await?;
    let srflx = srflx_candidate_line(fwd.public_addr, target);
    let answer = with_sdp(&answer, replace_candidates(&answer.sdp, &[srflx]))?;
    offerer.set_remote_description(answer).await?;

    wait_connected(&mut oe, CONNECT_TIMEOUT).await?;
    wait_connected(&mut ae, CONNECT_TIMEOUT).await?;
    rel_off.wait_open(Duration::from_secs(10)).await?;
    unrel_off.wait_open(Duration::from_secs(10)).await?;

    // Answerer side: route by label.
    let mut rel_ans = None;
    let mut unrel_ans = None;
    while rel_ans.is_none() || unrel_ans.is_none() {
        let dc = tokio::time::timeout(Duration::from_secs(10), ae.data_channels.recv())
            .await?
            .ok_or("answerer data channel stream ended")?;
        let label = dc.label().await?;
        println!(
            "answerer got '{label}': ordered={} max_retransmits={:?}",
            dc.ordered().await?,
            dc.max_retransmits().await?
        );
        match label.as_str() {
            "reliable" => rel_ans = Some(spawn_dc_reader(dc)),
            "unreliable" => unrel_ans = Some(spawn_dc_reader(dc)),
            other => return Err(format!("unexpected channel {other}").into()),
        }
    }
    let (mut rel_ans, mut unrel_ans) = (rel_ans.unwrap(), unrel_ans.unwrap());

    fwd.drop_every_nth.store(5, Ordering::Relaxed);

    // ── reliable ordered ──
    let t = Instant::now();
    for i in 0..N {
        reliable.send(message(i)).await?;
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    let mut got = Vec::new();
    while got.len() < N as usize {
        let m = rel_ans.recv(Duration::from_secs(30)).await?;
        assert_eq!(m.len(), MSG_LEN);
        got.push(index(&m));
    }
    let rel_time = t.elapsed();
    assert_eq!(got, (0..N).collect::<Vec<_>>(), "reliable: all, in order");

    // ── unreliable unordered, max_retransmits 0 ──
    let dropped_before = fwd.inbound_dropped.load(Ordering::Relaxed);
    for i in 0..N {
        unreliable.send(message(i)).await?;
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    let mut seen = Vec::new();
    // Nothing is retransmitted, so after 3 quiet seconds nothing more will come.
    while let Ok(m) = unrel_ans.recv(Duration::from_secs(3)).await {
        seen.push(index(&m));
    }
    let dropped_during = fwd.inbound_dropped.load(Ordering::Relaxed) - dropped_before;
    let unique: HashSet<u32> = seen.iter().copied().collect();
    let out_of_order = seen.windows(2).filter(|w| w[1] < w[0]).count();

    println!(
        "reliable: {N}/{N} in order in {rel_time:?}; unreliable: {} received ({} unique, \
         {out_of_order} out of order), {dropped_during} datagrams dropped meanwhile; \
         forwarder totals: fwd {} drop {}",
        seen.len(),
        unique.len(),
        fwd.inbound_forwarded.load(Ordering::Relaxed),
        fwd.inbound_dropped.load(Ordering::Relaxed),
    );
    assert_eq!(
        unique.len(),
        seen.len(),
        "no duplicates on the unreliable channel"
    );
    assert!(
        !seen.is_empty() && seen.len() < N as usize,
        "unreliable: some but not all delivered (got {})",
        seen.len()
    );

    offerer.close().await?;
    answerer.close().await?;
    Ok(())
}
