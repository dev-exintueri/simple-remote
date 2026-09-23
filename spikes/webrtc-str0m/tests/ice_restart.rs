// Spike only (throwaway).
//! Q2. Viewer (L) changes IP. Can we recover with an ICE restart on the same Rtc and
//! keep using the same data channel?
//!
//! Flow: connect, exchange data, then L's old address 127.0.0.1:10001 disappears
//! (network drops everything to/from it). Wait for L to report ICE Disconnected
//! (measures detection time), then L does `sdp_api().ice_restart(false)`, renegotiates,
//! adds a host candidate on 127.0.0.1:10011 and trickles it to R.
//!
//! Source facts (str0m 0.23.1 / is 0.11.0):
//! - change/sdp.rs `SdpApi::ice_restart(keep_local_candidates)`; with `false` the offer
//!   carries no candidates (sdp.rs `AsSdpParams::new`) and the local candidates are
//!   cleared when the answer is accepted (sdp.rs `update_ice` -> agent.rs `ice_restart`).
//!   So the new local candidate must be added AFTER `accept_answer`, then trickled.
//! - is agent.rs `add_remote_candidate` rejects a candidate whose ufrag differs from the
//!   current remote credentials; `add_local_candidate` stamps the new local ufrag, and
//!   `Candidate::to_sdp_string` includes it, so the trickled string carries the new ufrag.
mod common;

use std::time::Duration;

use common::{Sim, addr, both_connected, negotiate};
use str0m::change::{SdpAnswer, SdpOffer};
use str0m::{Candidate, Event, IceConnectionState, Rtc, RtcError};

const L_OLD: &str = "127.0.0.1:10001";
const L_NEW: &str = "127.0.0.1:10011";
const R_HOST: &str = "127.0.0.1:10002";

#[test]
fn ice_restart_after_viewer_ip_change_keeps_channel() -> Result<(), RtcError> {
    let now = std::time::Instant::now();
    let mut sim = Sim::new(now, Rtc::builder().build(now), Rtc::builder().build(now));
    sim.l_to_r.latency = Duration::from_millis(20);
    sim.r_to_l.latency = Duration::from_millis(20);

    sim.l
        .rtc
        .add_local_candidate(Candidate::host(addr(L_OLD), "udp").unwrap())
        .expect("L host");
    sim.r
        .rtc
        .add_local_candidate(Candidate::host(addr(R_HOST), "udp").unwrap())
        .expect("R host");

    let (cid, _, _) =
        negotiate(&mut sim.l.rtc, &mut sim.r.rtc, |api| api.add_channel("control".into()))?;

    sim.run_until(Duration::from_secs(10), Duration::from_millis(5), |s| {
        both_connected(s)
            && s.l.has_event(|e| matches!(e, Event::ChannelOpen(id, _) if *id == cid))
            && s.r.has_event(|e| matches!(e, Event::ChannelOpen(..)))
    })?
    .expect("initial connect + channel open");

    let r_cid = sim
        .r
        .events
        .iter()
        .find_map(|(_, e)| match e {
            Event::ChannelOpen(id, _) => Some(*id),
            _ => None,
        })
        .unwrap();

    assert!(sim.l.rtc.channel(cid).unwrap().write(false, b"before")?);
    sim.run_until(Duration::from_secs(5), Duration::from_millis(5), |s| {
        s.r.has_event(|e| matches!(e, Event::ChannelData(d) if d.data == b"before"))
    })?
    .expect("data before IP change");

    // --- IP change: old address vanishes ---
    let t_change = sim.elapsed();
    sim.blocked.push(addr(L_OLD));
    let n_events_l = sim.l.events.len();

    let detect = sim.run_until(Duration::from_secs(60), Duration::from_millis(10), |s| {
        s.l.events[n_events_l..].iter().any(|(_, e)| {
            matches!(e, Event::IceConnectionStateChange(IceConnectionState::Disconnected))
        })
    })?;
    println!("MEASURE ice_disconnect_detect_after_ip_change={detect:?}");
    println!(
        "OBSERVE after outage: l.is_alive={} l.is_connected={} r.is_connected={}",
        sim.l.rtc.is_alive(),
        sim.l.rtc.is_connected(),
        sim.r.rtc.is_connected()
    );

    // --- ICE restart from L on the SAME Rtc ---
    let t_restart = sim.elapsed();
    let (offer, pending) = {
        let mut api = sim.l.rtc.sdp_api();
        let creds = api.ice_restart(false);
        println!("OBSERVE new L ufrag={}", creds.ufrag);
        api.apply().expect("ice restart needs negotiation")
    };
    let offer_sdp = offer.to_sdp_string();
    assert!(
        !offer_sdp.contains("a=candidate"),
        "keep_local_candidates=false: restart offer carries no candidates"
    );
    let answer = sim
        .r
        .rtc
        .sdp_api()
        .accept_offer(SdpOffer::from_sdp_string(&offer_sdp)?)?;
    let answer_sdp = answer.to_sdp_string();
    sim.l
        .rtc
        .sdp_api()
        .accept_answer(pending, SdpAnswer::from_sdp_string(&answer_sdp)?)?;

    // New interface on L, trickled to R as a string (signaling).
    let cand = sim
        .l
        .rtc
        .add_local_candidate(Candidate::host(addr(L_NEW), "udp").unwrap())
        .expect("L new host")
        .clone();
    let trickle = cand.to_sdp_string();
    println!("OBSERVE trickled candidate: {trickle}");
    sim.r
        .rtc
        .add_remote_candidate(Candidate::from_sdp_string(&trickle).unwrap());

    let t_ice = sim
        .run_until(Duration::from_secs(10), Duration::from_millis(1), |s| both_connected(s))?
        .expect("reconnect within 10s after restart");

    assert!(sim.l.rtc.channel(cid).is_some(), "same ChannelId still valid on L");
    assert!(sim.l.rtc.channel(cid).unwrap().write(false, b"after-restart")?);
    let t_data = sim
        .run_until(Duration::from_secs(10), Duration::from_millis(1), |s| {
            s.r.has_event(
                |e| matches!(e, Event::ChannelData(d) if d.id == r_cid && d.data == b"after-restart"),
            )
        })?
        .expect("data after restart on same channel");

    assert!(sim.r.rtc.channel(r_cid).unwrap().write(false, b"ack")?);
    sim.run_until(Duration::from_secs(5), Duration::from_millis(1), |s| {
        s.l.has_event(|e| matches!(e, Event::ChannelData(d) if d.id == cid && d.data == b"ack"))
    })?
    .expect("reverse data after restart");

    println!("OBSERVE L last_tx={:?} R last_tx={:?}", sim.l.last_tx, sim.r.last_tx);
    assert_eq!(sim.l.last_tx.unwrap().0, addr(L_NEW), "L now sends from the new address");
    assert_eq!(sim.r.last_tx.unwrap().1, addr(L_NEW), "R now sends to the new address");
    assert!(
        !sim.l.has_event(|e| matches!(e, Event::ChannelClose(_))),
        "channel never closed"
    );

    println!(
        "MEASURE ip_change_at={t_change:?} restart_at={t_restart:?} \
         restart_to_ice_connected={t_ice:?} ice_connected_to_first_data={t_data:?}"
    );
    Ok(())
}
