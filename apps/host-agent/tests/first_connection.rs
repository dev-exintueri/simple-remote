use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use auth::{key_fingerprint, DeviceKeys, OneTimeCode, ViewerPake};
use host_agent::*;
use protocol::{control::RejectReason, pake_msg::{self, PakeMsg}, signaling::*, Timeouts, PROTOCOL_VERSION};
use transport::{fake_signal::FakeHub, Link};
use viewer_core::{connect, ConnectError, Side};

const LO: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
const HOST_ID: &str = "123456789";

fn cfg(version: u32) -> HostConfig {
    HostConfig { host_id: HOST_ID.into(), keys: DeviceKeys::generate("엄마 PC").unwrap(),
                 bind_ips: vec![LO], timeouts: Timeouts::default(), protocol_version: version }
}
type Seen = Arc<Mutex<Vec<ApprovalRequest>>>;
fn spawn_host(mut link: impl Link<ServerToHost, HostToServer> + Send + 'static, cfg: HostConfig,
              mut state: HostState, allow: bool, seen: Seen) -> JoinHandle<(ServeOutcome, HostState, HostConfig)> {
    // serve_next 전에 host 가 항상 모드를 알린다. 스레드를 나누기 전에 불러서, 이어지는
    // hub.viewer(JoinKind::New) 가 이 모드를 못 보고 지나가는 경쟁을 없앤다.
    set_mode(&mut link, HostMode::New).unwrap();
    thread::spawn(move || {
        let mut approve = |r: &ApprovalRequest| { seen.lock().unwrap().push(r.clone()); Some(allow) };
        let mut out = serve_next(&mut link, &cfg, &mut state, &mut approve).unwrap();
        if let ServeOutcome::Session(s) = &mut out { s.serve_pings(Duration::from_secs(3)).unwrap(); }
        (out, state, cfg)
    })
}
fn relays_of_host(hub: &FakeHub) -> Vec<PakeMsg> {
    hub.host_relays().iter().map(|b| pake_msg::decode(b).unwrap()).collect()
}

#[test]
fn first_connection_succeeds_and_rotates_code() {
    let (hub, host_link) = FakeHub::new();
    let state = HostState::new();
    let code = state.code.clone();
    let seen: Seen = Default::default();
    let h = spawn_host(host_link, cfg(PROTOCOL_VERSION), state, true, seen.clone());
    let vk = DeviceKeys::generate("노트북").unwrap();
    let mut s = connect(hub.viewer(JoinKind::New).unwrap(), HOST_ID, &code, &vk, &[LO], &Timeouts::default()).unwrap();
    assert!(s.ping(Duration::from_secs(2)).unwrap() < Duration::from_secs(1));
    let (out, state, cfg) = h.join().unwrap();
    assert!(matches!(out, ServeOutcome::Session(_)));
    assert_ne!(state.code.as_str(), code.as_str());
    assert_eq!(s.host.device_key, cfg.keys.device_public());
    let req = seen.lock().unwrap()[0].clone();
    assert_eq!(req.viewer_name, "노트북");
    assert_eq!(req.viewer_key_fingerprint, key_fingerprint(&vk.device_public()));
}

#[test]
fn wrong_code_leaks_no_addresses() {
    let (hub, host_link) = FakeHub::new();
    let h = spawn_host(host_link, cfg(PROTOCOL_VERSION), HostState::new(), true, Default::default());
    let wrong = OneTimeCode::parse("000000").unwrap(); // generate() 가 000000 을 낼 확률은 무시
    let r = connect(hub.viewer(JoinKind::New).unwrap(), HOST_ID, &wrong, &DeviceKeys::generate("v").unwrap(), &[LO], &Timeouts::default());
    assert!(matches!(r, Err(ConnectError::WrongCode)));
    let (out, state, _) = h.join().unwrap();
    assert!(matches!(out, ServeOutcome::Failed(AttemptFailure::NoConfirm)));
    assert!(state.limiter.check(Instant::now()).is_err()); // 떠난 viewer 도 실패로 기록됨
    assert!(!relays_of_host(&hub).iter().any(|m| matches!(m, PakeMsg::Answer { .. })));
}

#[test]
fn retry_after_during_backoff() {
    let (hub, mut host_link) = FakeHub::new();
    let (c, mut state) = (cfg(PROTOCOL_VERSION), HostState::new());
    let wrong = OneTimeCode::parse("000000").unwrap();
    let vk = DeviceKeys::generate("v").unwrap();
    set_mode(&mut host_link, HostMode::New).unwrap();
    let hub2 = hub.clone();
    let v = thread::spawn(move || {
        let first = connect(hub2.viewer(JoinKind::New).unwrap(), HOST_ID, &wrong, &vk, &[LO], &Timeouts::default());
        let second = connect(hub2.viewer(JoinKind::New).unwrap(), HOST_ID, &wrong, &vk, &[LO], &Timeouts::default());
        (first, second)
    });
    let mut deny = |_: &ApprovalRequest| Some(false);
    serve_next(&mut host_link, &c, &mut state, &mut deny).unwrap();
    let replies_before = relays_of_host(&hub).iter().filter(|m| matches!(m, PakeMsg::Reply { .. })).count();
    let out = serve_next(&mut host_link, &c, &mut state, &mut deny).unwrap();
    assert!(matches!(out, ServeOutcome::Failed(AttemptFailure::WaitRequired)));
    let (first, second) = v.join().unwrap();
    assert!(matches!(first, Err(ConnectError::WrongCode)));
    match second { Err(ConnectError::RetryAfter(d)) => assert!(d > Duration::ZERO && d <= Duration::from_secs(1)), other => panic!("{other:?}") }
    let replies_after = relays_of_host(&hub).iter().filter(|m| matches!(m, PakeMsg::Reply { .. })).count();
    assert_eq!(replies_before, replies_after); // 대기 중에는 PAKE 응답이 없음
}

#[test]
fn approval_denied_keeps_code() {
    let (hub, host_link) = FakeHub::new();
    let state = HostState::new();
    let code = state.code.clone();
    let h = spawn_host(host_link, cfg(PROTOCOL_VERSION), state, false, Default::default());
    let r = connect(hub.viewer(JoinKind::New).unwrap(), HOST_ID, &code, &DeviceKeys::generate("v").unwrap(), &[LO], &Timeouts::default());
    assert!(matches!(r, Err(ConnectError::Rejected(RejectReason::Denied))));
    let (out, state, _) = h.join().unwrap();
    assert!(matches!(out, ServeOutcome::Failed(AttemptFailure::Denied)));
    assert_eq!(state.code.as_str(), code.as_str());
}

#[test]
fn version_mismatch_reports_older_side() {
    let (hub, host_link) = FakeHub::new();
    let state = HostState::new();
    let code = state.code.clone();
    let _h = spawn_host(host_link, cfg(PROTOCOL_VERSION + 1), state, true, Default::default());
    let r = connect(hub.viewer(JoinKind::New).unwrap(), HOST_ID, &code, &DeviceKeys::generate("v").unwrap(), &[LO], &Timeouts::default());
    assert!(matches!(r, Err(ConnectError::VersionMismatch { older: Side::Viewer, .. })));
}

#[test]
fn host_leaves_mid_handshake() {
    let (hub, mut host_link) = FakeHub::new();
    let h = thread::spawn(move || {
        assert_eq!(
            host_link.recv(Duration::from_secs(5)).unwrap(),
            Some(ServerToHost::ViewerJoined { n: 1, kind: JoinKind::Reconnect })
        );
        assert!(matches!(host_link.recv(Duration::from_secs(5)).unwrap(), Some(ServerToHost::Relay { .. })));
        drop(host_link); // Start 를 받고 사라짐
    });
    let start = Instant::now();
    // 이 테스트는 session.rs 를 거치지 않고 signaling 메시지만 직접 보므로, 모드 설정 없이
    // 항상 허용되는 kind::Reconnect 를 쓴다.
    let r = connect(hub.viewer(JoinKind::Reconnect).unwrap(), HOST_ID, &OneTimeCode::generate(), &DeviceKeys::generate("v").unwrap(), &[LO], &Timeouts::default());
    h.join().unwrap();
    assert!(matches!(r, Err(ConnectError::HostLeft)));
    assert!(start.elapsed() < Timeouts::default().signaling_step);
}

#[test]
fn garbage_confirm_counts_as_failure() {
    let (hub, host_link) = FakeHub::new();
    let state = HostState::new();
    let code = state.code.clone();
    let h = spawn_host(host_link, cfg(PROTOCOL_VERSION), state, true, Default::default());
    let mut v = hub.viewer(JoinKind::New).unwrap();
    assert_eq!(v.recv(Duration::from_secs(5)).unwrap(), Some(ServerToViewer::Joined));
    let (_vp, pa) = ViewerPake::start(&code, HOST_ID).unwrap();
    v.send(&ViewerToServer::Relay { data: hex::encode(pake_msg::encode(&PakeMsg::Start { pa })) }).unwrap();
    assert!(matches!(v.recv(Duration::from_secs(5)).unwrap(), Some(ServerToViewer::Relay { .. }))); // Reply
    let bad = PakeMsg::Confirm { mac_a: vec![0; 32], sealed_offer: vec![1; 10] };
    v.send(&ViewerToServer::Relay { data: hex::encode(pake_msg::encode(&bad)) }).unwrap();
    let (out, state, _) = h.join().unwrap();
    assert!(matches!(out, ServeOutcome::Failed(AttemptFailure::WrongCode)));
    assert!(state.limiter.check(Instant::now()).is_err());
}

#[test]
fn approval_times_out_as_no_response() {
    let (hub, mut host_link) = FakeHub::new();
    let mut c = cfg(PROTOCOL_VERSION);
    c.timeouts.approval = Duration::from_secs(1);
    let mut state = HostState::new();
    let code = state.code.clone();
    set_mode(&mut host_link, HostMode::New).unwrap();
    let h = thread::spawn(move || {
        let mut never = |_: &ApprovalRequest| None;
        let out = serve_next(&mut host_link, &c, &mut state, &mut never).unwrap();
        (out, state)
    });
    let r = connect(hub.viewer(JoinKind::New).unwrap(), HOST_ID, &code, &DeviceKeys::generate("v").unwrap(), &[LO], &Timeouts::default());
    assert!(matches!(r, Err(ConnectError::Rejected(RejectReason::NoResponse))));
    let (out, state) = h.join().unwrap();
    assert!(matches!(out, ServeOutcome::Failed(AttemptFailure::Denied)));
    assert_eq!(state.code.as_str(), code.as_str());
}

#[test]
fn slow_approval_keeps_connection() {
    let (hub, mut host_link) = FakeHub::new();
    let c = cfg(PROTOCOL_VERSION);
    let mut state = HostState::new();
    let code = state.code.clone();
    set_mode(&mut host_link, HostMode::New).unwrap();
    let h = thread::spawn(move || {
        let mut first_call: Option<Instant> = None;
        let mut slow = |_: &ApprovalRequest| {
            let first = *first_call.get_or_insert_with(Instant::now);
            (first.elapsed() >= Duration::from_secs(2)).then_some(true)
        };
        let mut out = serve_next(&mut host_link, &c, &mut state, &mut slow).unwrap();
        if let ServeOutcome::Session(s) = &mut out { s.serve_pings(Duration::from_secs(3)).unwrap(); }
        out
    });
    let mut s = connect(hub.viewer(JoinKind::New).unwrap(), HOST_ID, &code, &DeviceKeys::generate("v").unwrap(), &[LO], &Timeouts::default()).unwrap();
    assert!(s.ping(Duration::from_secs(2)).unwrap() < Duration::from_secs(2));
    assert!(matches!(h.join().unwrap(), ServeOutcome::Session(_)));
}
