use std::net::{IpAddr, Ipv4Addr};
use std::time::{Duration, Instant};
use transport::{Peer, PeerEvent};

const LO: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

fn pump_until(a: &mut Peer, b: &mut Peer, secs: u64, mut done: impl FnMut(&[PeerEvent], &[PeerEvent]) -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(secs);
    let (mut ea, mut eb) = (vec![], vec![]);
    while Instant::now() < end {
        match a.poll(Duration::from_millis(5)) { Ok(v) => ea.extend(v), Err(_) => return false }
        match b.poll(Duration::from_millis(5)) { Ok(v) => eb.extend(v), Err(_) => return false }
        if done(&ea, &eb) { return true; }
    }
    false
}

#[test]
fn loopback_roundtrip() {
    let (mut v, mut h) = (Peer::new(&[LO]).unwrap(), Peer::new(&[LO]).unwrap());
    let (offer, pending) = v.create_offer().unwrap();
    let answer = h.accept_offer(&offer).unwrap();
    v.accept_answer(pending, &answer).unwrap();
    assert!(pump_until(&mut v, &mut h, 10, |a, b| a.contains(&PeerEvent::ChannelOpen) && b.contains(&PeerEvent::ChannelOpen)));
    assert_eq!(v.remote_fingerprint().unwrap(), h.local_fingerprint());
    v.send(b"ping").unwrap();
    assert!(pump_until(&mut v, &mut h, 5, |_, b| b.contains(&PeerEvent::Data(b"ping".to_vec()))));
}

#[test]
fn fingerprint_mismatch_never_opens() {
    let (mut v, mut h, mut other) = (Peer::new(&[LO]).unwrap(), Peer::new(&[LO]).unwrap(), Peer::new(&[LO]).unwrap());
    let (offer, pending) = v.create_offer().unwrap();
    let answer = h.accept_offer(&offer).unwrap();
    let (other_offer, _) = other.create_offer().unwrap();
    let fake = replace_fingerprint(&answer, &other_offer); // answer 의 a=fingerprint 줄을 other 의 것으로
    v.accept_answer(pending, &fake).unwrap();
    assert!(!pump_until(&mut v, &mut h, 5, |a, _| a.contains(&PeerEvent::ChannelOpen)));
}

fn replace_fingerprint(sdp: &str, from: &str) -> String {
    let fp = from.lines().find(|l| l.starts_with("a=fingerprint:")).unwrap();
    sdp.lines().map(|l| if l.starts_with("a=fingerprint:") { fp } else { l }).collect::<Vec<_>>().join("\r\n") + "\r\n"
}

#[test]
fn local_ips_excludes_loopback() {
    assert!(transport::local_ips().iter().all(|ip| !ip.is_loopback() && !ip.is_unspecified()));
}
