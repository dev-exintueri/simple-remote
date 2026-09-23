// Spike only (throwaway).
//! Q1. Can the host advertise a UPnP-mapped (external) address so the viewer connects through it?
//!
//! Simulation: the answerer ("host") binds a fixed 127.0.0.1:<port>. A NAT-like forwarder owns
//! 127.0.0.2:40000 (the "router's external port", Linux routes all of 127/8 to lo without setup)
//! and relays to the answerer. The answerer's SDP loses its host candidate and gains
//! `typ srflx 127.0.0.2 40000 raddr 127.0.0.1 rport <port>` instead.
//!
//! Why SDP editing: the async `webrtc` 0.21.0 `PeerConnection` trait has no
//! `add_local_candidate` (only the sans-I/O `rtc` core does, and the driver owns that core), and
//! `SettingEngineBuilder::with_nat_1to1_ips` is stored but never read anywhere in `rtc` or
//! `webrtc` 0.21.0 (and could only swap the IP, not the port, anyway).
//!
//! The offerer's host candidates are stripped as well, so the answerer cannot check the
//! offerer directly: the only possible path is offerer → forwarder → answerer. The answerer
//! learns the forwarder's inside address as a peer-reflexive candidate from incoming checks.

mod common;

use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use common::*;

const PUBLIC: &str = "127.0.0.2:40000";

#[tokio::test(flavor = "multi_thread")]
async fn srflx_candidate_injected_via_sdp_connects_through_forwarder() -> TestResult {
    let port = free_udp_port("127.0.0.1");
    let answerer_addr: SocketAddr = format!("127.0.0.1:{port}").parse()?;
    let fwd = Forwarder::start(PUBLIC, answerer_addr).await?;

    let (offerer, mut oe) =
        build_peer("offerer", "127.0.0.1:0", None, setting_engine().build()).await?;
    let (answerer, mut ae) = build_peer(
        "answerer",
        &answerer_addr.to_string(),
        None,
        setting_engine().build(),
    )
    .await?;

    let dc = offerer.create_data_channel("q1", None).await?;
    let mut off_reader = spawn_dc_reader(dc.clone());

    let t0 = Instant::now();
    let offer = make_offer(&offerer, &mut oe, None).await?;
    let offer = with_sdp(&offer, strip_candidates(&offer.sdp))?;

    let answer = make_answer(&answerer, &mut ae, offer).await?;
    let hosts = host_candidates(&answer.sdp);
    println!("answerer host candidates: {hosts:?}");
    assert_eq!(hosts, vec![answerer_addr], "answerer bound the fixed port");
    let srflx = srflx_candidate_line(fwd.public_addr, answerer_addr);
    println!("injected: {srflx}");
    let answer = with_sdp(&answer, replace_candidates(&answer.sdp, &[srflx]))?;
    println!("answer as sent:\n{}", answer.sdp);
    offerer.set_remote_description(answer).await?;

    wait_connected(&mut oe, CONNECT_TIMEOUT).await?;
    wait_connected(&mut ae, CONNECT_TIMEOUT).await?;
    println!("connected in {:?}", t0.elapsed());

    off_reader.wait_open(Duration::from_secs(10)).await?;
    let adc = tokio::time::timeout(Duration::from_secs(10), ae.data_channels.recv())
        .await?
        .ok_or("no data channel on answerer")?;
    let mut ans_reader = spawn_dc_reader(adc.clone());

    dc.send_text("via-upnp").await?;
    let got = ans_reader.recv(Duration::from_secs(10)).await?;
    assert_eq!(got, b"via-upnp");
    adc.send_text("reply").await?;
    let got = off_reader.recv(Duration::from_secs(10)).await?;
    assert_eq!(got, b"reply");

    let off_pair = selected_pair(&offerer).await?;
    let ans_pair = selected_pair(&answerer).await?;
    println!("offerer selected pair: {off_pair:?}");
    println!("answerer selected pair: {ans_pair:?}");
    let inbound = fwd.inbound_forwarded.load(Ordering::Relaxed);
    let outbound = fwd.outbound_forwarded.load(Ordering::Relaxed);
    println!("forwarder datagrams: inbound {inbound}, outbound {outbound}");

    assert!(
        inbound > 0 && outbound > 0,
        "traffic went through the forwarder"
    );
    let off_pair = off_pair.ok_or("offerer has no selected pair")?;
    assert!(
        off_pair.contains("remote 127.0.0.2:40000"),
        "offerer's selected remote is the injected srflx address: {off_pair}"
    );

    offerer.close().await?;
    answerer.close().await?;
    Ok(())
}
