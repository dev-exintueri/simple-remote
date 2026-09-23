// Spike only (throwaway).
//! Q1. Can the host side (R) advertise only a server-reflexive candidate that it
//! learned out of band (for example a UPnP-mapped public address) and still connect?
//!
//! Network: R's socket is bound to BASE (127.0.0.1:10002). A NAT maps it to
//! MAPPED (127.0.0.2:40000). Packets to MAPPED reach R with destination=BASE;
//! packets from BASE appear to L as coming from MAPPED.
//!
//! Source facts this test relies on (is-0.11.0):
//! - agent.rs `stun_server_handle_request`: an incoming STUN request is only accepted
//!   when its destination equals a local candidate of kind Host or Relayed. A srflx
//!   candidate alone therefore cannot receive L's checks: R must ALSO hold a local host
//!   candidate for BASE. We add that host candidate after the SDP answer was created,
//!   so it is never signalled (L only ever learns MAPPED).
//! - agent.rs `form_pairs`: pairs whose local candidates share the same base and whose
//!   remote is identical are redundant; the higher priority (host) one is kept.
//!   Sending always uses `local.base()` (agent.rs NominatedSend), so R sends from BASE.
//! - Selected pair observability: there is no dedicated event. `Event::PeerStats`
//!   (enabled by `RtcConfig::set_stats_interval`) carries `selected_candidate_pair`
//!   built from the nominated send address (lib.rs `do_handle_timeout`): local.addr is
//!   the send *base*, remote.addr the destination. The last `Output::Transmit`
//!   (source, destination) is a second, direct observation.
mod common;

use std::time::Duration;

use common::{Sim, addr, both_connected, negotiate};
use str0m::{Candidate, Event, Rtc, RtcError};

const L_HOST: &str = "127.0.0.1:10001";
const R_BASE: &str = "127.0.0.1:10002";
const R_MAPPED: &str = "127.0.0.2:40000";

fn build_rtc(now: std::time::Instant) -> Rtc {
    Rtc::builder()
        .set_stats_interval(Some(Duration::from_millis(500)))
        .build(now)
}

#[test]
fn srflx_only_advertised_connects_through_nat() -> Result<(), RtcError> {
    let now = std::time::Instant::now();
    let mut sim = Sim::new(now, build_rtc(now), build_rtc(now));
    sim.nat.push((addr(R_BASE), addr(R_MAPPED)));
    sim.l_to_r.latency = Duration::from_millis(10);
    sim.r_to_l.latency = Duration::from_millis(10);

    sim.l
        .rtc
        .add_local_candidate(Candidate::host(addr(L_HOST), "udp").unwrap())
        .expect("L host accepted");
    let srflx = Candidate::server_reflexive(addr(R_MAPPED), addr(R_BASE), "udp").unwrap();
    sim.r
        .rtc
        .add_local_candidate(srflx)
        .expect("R srflx accepted");

    let (cid, offer_sdp, answer_sdp) =
        negotiate(&mut sim.l.rtc, &mut sim.r.rtc, |api| api.add_channel("control".into()))?;
    println!("--- offer ---\n{offer_sdp}--- answer ---\n{answer_sdp}---");
    assert!(answer_sdp.contains("typ srflx"), "answer must carry the srflx candidate");
    assert!(!answer_sdp.contains("typ host"), "answer must not carry a host candidate");
    assert!(answer_sdp.contains("127.0.0.2 40000"), "srflx address in answer");

    // Needed so R's agent accepts STUN requests arriving at BASE (see header comment).
    // Not signalled: the answer is already out.
    sim.r
        .rtc
        .add_local_candidate(Candidate::host(addr(R_BASE), "udp").unwrap())
        .expect("R local host (base) accepted");

    let t_conn = sim
        .run_until(Duration::from_secs(10), Duration::from_millis(5), |s| both_connected(s))?
        .expect("ICE+DTLS connect within 10s");
    println!("MEASURE connect_time={t_conn:?}");

    let open = sim.run_until(Duration::from_secs(5), Duration::from_millis(5), |s| {
        s.l.has_event(|e| matches!(e, Event::ChannelOpen(id, _) if *id == cid))
            && s.r.has_event(|e| matches!(e, Event::ChannelOpen(..)))
    })?;
    assert!(open.is_some(), "data channel opens on both sides");

    let ok = sim.l.rtc.channel(cid).expect("L channel").write(false, b"hello-srflx")?;
    assert!(ok);
    let got = sim.run_until(Duration::from_secs(5), Duration::from_millis(5), |s| {
        s.r.has_event(|e| matches!(e, Event::ChannelData(d) if d.data == b"hello-srflx"))
    })?;
    assert!(got.is_some(), "R receives data over the srflx path");

    let r_cid = sim
        .r
        .events
        .iter()
        .find_map(|(_, e)| match e {
            Event::ChannelOpen(id, _) => Some(*id),
            _ => None,
        })
        .unwrap();
    let ok = sim.r.rtc.channel(r_cid).expect("R channel").write(false, b"reply")?;
    assert!(ok);
    let got = sim.run_until(Duration::from_secs(5), Duration::from_millis(5), |s| {
        s.l.has_event(|e| matches!(e, Event::ChannelData(d) if d.data == b"reply"))
    })?;
    assert!(got.is_some(), "L receives R's reply");

    // Let at least one stats interval pass after nomination.
    sim.run_for(Duration::from_secs(1))?;

    // Direct observation: where each side actually sends.
    println!("OBSERVE L last_tx={:?}", sim.l.last_tx);
    println!("OBSERVE R last_tx={:?}", sim.r.last_tx);
    assert_eq!(sim.l.last_tx.unwrap().1, addr(R_MAPPED), "L sends to the mapped address");
    assert_eq!(sim.r.last_tx.unwrap().0, addr(R_BASE), "R sends from its base socket");

    // Stats observation of the selected pair.
    let pair = |peer: &common::Peer| {
        peer.events.iter().rev().find_map(|(_, e)| match e {
            Event::PeerStats(s) => s.selected_candidate_pair.clone(),
            _ => None,
        })
    };
    let l_pair = pair(&sim.l).expect("L PeerStats with selected pair");
    let r_pair = pair(&sim.r).expect("R PeerStats with selected pair");
    println!(
        "OBSERVE L selected local={} remote={}",
        l_pair.local.addr, l_pair.remote.addr
    );
    println!(
        "OBSERVE R selected local={} remote={}",
        r_pair.local.addr, r_pair.remote.addr
    );
    assert_eq!(l_pair.local.addr, addr(L_HOST));
    assert_eq!(l_pair.remote.addr, addr(R_MAPPED), "L's selected remote is the srflx");
    assert_eq!(r_pair.local.addr, addr(R_BASE), "R reports the base as local (send socket)");
    assert_eq!(r_pair.remote.addr, addr(L_HOST));

    println!(
        "MEASURE l_to_r={:?} r_to_l={:?}",
        sim.l_to_r.stats, sim.r_to_l.stats
    );
    Ok(())
}

/// Same setup but WITHOUT the local host candidate for BASE on R.
/// Expected from source (agent.rs `stun_server_handle_request`): R discards L's checks
/// ("Discarding STUN request on unknown interface"), L (controlling) never gets a
/// successful pair, so nothing is nominated and the connection does not come up.
/// This documents that the host app must keep a host candidate for the socket that
/// the UPnP mapping points at, even if it chooses not to signal it.
#[test]
fn srflx_without_local_host_base_does_not_connect() -> Result<(), RtcError> {
    let now = std::time::Instant::now();
    let mut sim = Sim::new(now, build_rtc(now), build_rtc(now));
    sim.nat.push((addr(R_BASE), addr(R_MAPPED)));

    sim.l
        .rtc
        .add_local_candidate(Candidate::host(addr(L_HOST), "udp").unwrap())
        .expect("L host accepted");
    sim.r
        .rtc
        .add_local_candidate(
            Candidate::server_reflexive(addr(R_MAPPED), addr(R_BASE), "udp").unwrap(),
        )
        .expect("R srflx accepted");

    let _ = negotiate(&mut sim.l.rtc, &mut sim.r.rtc, |api| api.add_channel("control".into()))?;

    let t = sim.run_until(Duration::from_secs(10), Duration::from_millis(5), |s| both_connected(s))?;
    println!(
        "MEASURE connected_within_10s={} l_to_r={:?} r_to_l={:?}",
        t.is_some(),
        sim.l_to_r.stats,
        sim.r_to_l.stats
    );
    assert!(t.is_none(), "expected no connection without a local host candidate at BASE");
    Ok(())
}
