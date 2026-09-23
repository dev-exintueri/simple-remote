// Spike only (throwaway).
//! Q4. Reliable-ordered vs unordered/MaxRetransmits{0} data channels under loss.
//!
//! After both channels are open, every 5th packet is dropped in each direction
//! (deterministic). L sends 200 messages on each channel, one pair every 5 ms.
//! Expect: reliable gets all 200 in order; unreliable gets some but not all.
//!
//! Source facts (str0m 0.23.1): `ChannelConfig { label, ordered, reliability,
//! negotiated, protocol }` and `Reliability::{Reliable, MaxPacketLifetime{lifetime},
//! MaxRetransmits{retransmits}}` in sctp/mod.rs, re-exported from `str0m::channel`.
//! `SdpApi::add_channel_with_config` in change/sdp.rs. `Channel::write(binary, buf)
//! -> Result<bool>` (false = not accepted, buffer full) in channel.rs.
mod common;

use std::time::Duration;

use common::{Sim, addr, both_connected, negotiate};
use str0m::channel::{ChannelConfig, ChannelId, Reliability};
use str0m::{Candidate, Event, Rtc, RtcError};

const N: usize = 200;

fn msg(prefix: u8, i: usize) -> Vec<u8> {
    let mut v = format!("{}{:03}", prefix as char, i).into_bytes();
    v.resize(300, b'.');
    v
}

fn seq_of(data: &[u8]) -> usize {
    std::str::from_utf8(&data[1..4]).unwrap().parse().unwrap()
}

#[test]
fn reliable_vs_unreliable_under_loss() -> Result<(), RtcError> {
    let now = std::time::Instant::now();
    let mut sim = Sim::new(now, Rtc::builder().build(now), Rtc::builder().build(now));
    sim.l_to_r.latency = Duration::from_millis(15);
    sim.r_to_l.latency = Duration::from_millis(15);
    sim.l
        .rtc
        .add_local_candidate(Candidate::host(addr("127.0.0.1:10001"), "udp").unwrap())
        .unwrap();
    sim.r
        .rtc
        .add_local_candidate(Candidate::host(addr("127.0.0.1:10002"), "udp").unwrap())
        .unwrap();

    let ((rel, unrel), _, _) = negotiate(&mut sim.l.rtc, &mut sim.r.rtc, |api| {
        let rel = api.add_channel_with_config(ChannelConfig {
            label: "reliable".into(),
            ..Default::default()
        });
        let unrel = api.add_channel_with_config(ChannelConfig {
            label: "unreliable".into(),
            ordered: false,
            reliability: Reliability::MaxRetransmits { retransmits: 0 },
            ..Default::default()
        });
        (rel, unrel)
    })?;

    let r_id = |s: &Sim, label: &str| -> Option<ChannelId> {
        s.r.events.iter().find_map(|(_, e)| match e {
            Event::ChannelOpen(id, l) if l == label => Some(*id),
            _ => None,
        })
    };

    sim.run_until(Duration::from_secs(10), Duration::from_millis(5), |s| {
        both_connected(s)
            && s.l.has_event(|e| matches!(e, Event::ChannelOpen(id, _) if *id == rel))
            && s.l.has_event(|e| matches!(e, Event::ChannelOpen(id, _) if *id == unrel))
            && r_id(s, "reliable").is_some()
            && r_id(s, "unreliable").is_some()
    })?
    .expect("both channels open");
    let r_rel = r_id(&sim, "reliable").unwrap();
    let r_unrel = r_id(&sim, "unreliable").unwrap();

    let cfg = sim.r.rtc.channel(r_unrel).unwrap().config().cloned();
    println!("OBSERVE R view of unreliable channel config: {cfg:?}");

    // Loss starts now.
    sim.l_to_r.drop_every_nth = Some(5);
    sim.r_to_l.drop_every_nth = Some(5);
    let t0 = sim.elapsed();

    for i in 0..N {
        assert!(sim.l.rtc.channel(rel).unwrap().write(true, &msg(b'r', i))?, "rel write {i}");
        assert!(sim.l.rtc.channel(unrel).unwrap().write(true, &msg(b'u', i))?, "unrel write {i}");
        sim.run_for(Duration::from_millis(5))?;
    }

    let count = |s: &Sim, id: ChannelId| {
        s.r.events
            .iter()
            .filter(|(_, e)| matches!(e, Event::ChannelData(d) if d.id == id))
            .count()
    };
    let done = sim.run_until(Duration::from_secs(60), Duration::from_millis(10), |s| {
        count(s, r_rel) >= N
    })?;
    // Give unreliable stragglers a moment.
    sim.run_for(Duration::from_secs(1))?;

    let rel_seq: Vec<usize> = sim
        .r
        .events
        .iter()
        .filter_map(|(_, e)| match e {
            Event::ChannelData(d) if d.id == r_rel => Some(seq_of(&d.data)),
            _ => None,
        })
        .collect();
    let unrel_seq: Vec<usize> = sim
        .r
        .events
        .iter()
        .filter_map(|(_, e)| match e {
            Event::ChannelData(d) if d.id == r_unrel => Some(seq_of(&d.data)),
            _ => None,
        })
        .collect();
    let last_rel_at = sim
        .r
        .events
        .iter()
        .filter(|(_, e)| matches!(e, Event::ChannelData(d) if d.id == r_rel))
        .map(|(t, _)| *t)
        .last();

    println!(
        "MEASURE reliable_received={} unreliable_received={} all_reliable_after={:?} \
         last_reliable_at={:?} (loss start {:?})",
        rel_seq.len(),
        unrel_seq.len(),
        done,
        last_rel_at,
        t0
    );
    println!("MEASURE l_to_r={:?} r_to_l={:?}", sim.l_to_r.stats, sim.r_to_l.stats);
    let mut missing: Vec<usize> = (0..N).filter(|i| !unrel_seq.contains(i)).collect();
    missing.truncate(30);
    println!("OBSERVE first missing unreliable seqs: {missing:?}");

    assert_eq!(rel_seq, (0..N).collect::<Vec<_>>(), "reliable: all 200, in order");
    assert!(unrel_seq.len() < N, "unreliable must lose something under 20% loss");
    assert!(!unrel_seq.is_empty(), "unreliable must deliver something");
    let mut dedup = unrel_seq.clone();
    dedup.sort();
    dedup.dedup();
    assert_eq!(dedup.len(), unrel_seq.len(), "no duplicates on unreliable");
    assert!(sim.l.rtc.is_connected() && sim.r.rtc.is_connected());
    Ok(())
}
