// Spike only (throwaway).
//! Q2. Does ICE restart recover the connection in place, keeping the same DataChannel, and how
//! long does it take on loopback?
//!
//! Three variants:
//! - `restart_ice()` then `create_offer(None)` (the W3C restartIce path),
//! - `create_offer(Some(RTCOfferOptions { ice_restart: true }))`,
//! - `restart_ice()` with `with_discard_local_candidates_during_ice_restart(true)`, which also
//!   replaces the UDP sockets (the "network changed" case).
//!
//! Each asserts: new ICE ufrag, state back to Connected, and the data channel created before
//! the restart delivers messages both ways after it.

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::*;
use rtc::peer_connection::configuration::RTCOfferOptions;
use webrtc::peer_connection::{PeerConnection, RTCPeerConnectionState};

#[derive(Clone, Copy, Debug)]
enum How {
    RestartIceApi,
    OfferOption,
}

async fn local_ufrag(pc: &Arc<dyn PeerConnection>) -> TestResult<String> {
    let sctp = pc.sctp().await.ok_or("no SCTP transport")?;
    let params = sctp
        .transport()
        .ice_transport()
        .get_local_parameters()
        .await?
        .ok_or("no local ICE parameters")?;
    Ok(params.username_fragment)
}

async fn run(how: How, discard_local_candidates: bool) -> TestResult {
    let se = || {
        setting_engine()
            .with_discard_local_candidates_during_ice_restart(discard_local_candidates)
            .build()
    };
    let (offerer, mut oe) = build_peer("offerer", "127.0.0.1:0", None, se()).await?;
    let (answerer, mut ae) = build_peer("answerer", "127.0.0.1:0", None, se()).await?;

    let dc = offerer.create_data_channel("q2", None).await?;
    let mut off_reader = spawn_dc_reader(dc.clone());

    let t0 = Instant::now();
    negotiate(&offerer, &mut oe, &answerer, &mut ae, None).await?;
    wait_connected(&mut oe, CONNECT_TIMEOUT).await?;
    wait_connected(&mut ae, CONNECT_TIMEOUT).await?;
    off_reader.wait_open(Duration::from_secs(10)).await?;
    let adc = tokio::time::timeout(Duration::from_secs(10), ae.data_channels.recv())
        .await?
        .ok_or("no data channel on answerer")?;
    let mut ans_reader = spawn_dc_reader(adc.clone());
    println!("initial connect + open: {:?}", t0.elapsed());

    dc.send_text("before").await?;
    assert_eq!(ans_reader.recv(Duration::from_secs(5)).await?, b"before");
    adc.send_text("before-reply").await?;
    assert_eq!(
        off_reader.recv(Duration::from_secs(5)).await?,
        b"before-reply"
    );

    let ufrag_before = local_ufrag(&offerer).await?;
    println!("pair before: {:?}", selected_pair(&offerer).await?);

    // ── restart ──
    let t_restart = Instant::now();
    let options = match how {
        How::RestartIceApi => {
            offerer.restart_ice().await?;
            None
        }
        How::OfferOption => Some(RTCOfferOptions { ice_restart: true }),
    };
    negotiate(&offerer, &mut oe, &answerer, &mut ae, options).await?;
    let t_signaled = t_restart.elapsed();

    // Wait for Connected again (it may never have left Connected on loopback; then this
    // returns at once and the message round trip below is the real proof).
    wait_connected(&mut oe, CONNECT_TIMEOUT).await?;
    wait_connected(&mut ae, CONNECT_TIMEOUT).await?;

    dc.send_text("after").await?;
    assert_eq!(ans_reader.recv(Duration::from_secs(15)).await?, b"after");
    let t_first_msg = t_restart.elapsed();
    adc.send_text("after-reply").await?;
    assert_eq!(
        off_reader.recv(Duration::from_secs(15)).await?,
        b"after-reply"
    );
    let t_round_trip = t_restart.elapsed();

    let ufrag_after = local_ufrag(&offerer).await?;
    println!("pair after: {:?}", selected_pair(&offerer).await?);
    println!(
        "[{how:?}, discard={discard_local_candidates}] ufrag {ufrag_before} -> {ufrag_after}; \
         signaling done {t_signaled:?}, first msg {t_first_msg:?}, round trip {t_round_trip:?}"
    );
    print_history(&oe, t0);
    print_history(&ae, t0);

    assert_ne!(ufrag_before, ufrag_after, "the offer really restarted ICE");
    assert_eq!(*oe.state.borrow(), RTCPeerConnectionState::Connected);
    assert_eq!(*ae.state.borrow(), RTCPeerConnectionState::Connected);

    offerer.close().await?;
    answerer.close().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn restart_via_restart_ice_api() -> TestResult {
    run(How::RestartIceApi, false).await
}

#[tokio::test(flavor = "multi_thread")]
async fn restart_via_offer_options() -> TestResult {
    run(How::OfferOption, false).await
}

#[tokio::test(flavor = "multi_thread")]
async fn restart_with_socket_rebind() -> TestResult {
    run(How::RestartIceApi, true).await
}
