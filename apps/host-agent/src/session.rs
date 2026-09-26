//! host 쪽 첫 연결 흐름: viewer 하나를 받아 PAKE, 봉인한 SDP 교환, Hello 확인, 허락까지 처리한다.

use std::net::IpAddr;
use std::time::{Duration, Instant};

use auth::{DeviceKeys, HostPake, OneTimeCode, PeerIdentity, Role, SessionKeys, key_fingerprint, verify_hello};
use protocol::control::{Control, Decision, RejectReason};
use protocol::pake_msg::{self, PakeMsg};
use protocol::signaling::{HostToServer, ServerToHost};
use protocol::{Stage, Timeouts};
use transport::{Channel, Link, LinkError, Next, Opened, Peer, TransportError};

use crate::AttemptLimiter;

#[derive(Debug)]
pub struct HostConfig {
    pub host_id: String,
    pub keys: DeviceKeys,
    pub bind_ips: Vec<IpAddr>,
    pub timeouts: Timeouts,
    /// 보통 `PROTOCOL_VERSION`. 버전 불일치 테스트에서만 바꾼다.
    pub protocol_version: u32,
}

#[derive(Debug)]
pub struct HostState {
    pub code: OneTimeCode,
    pub limiter: AttemptLimiter,
}

impl HostState {
    pub fn new() -> HostState {
        HostState { code: OneTimeCode::generate(), limiter: AttemptLimiter::new() }
    }
}

impl Default for HostState {
    fn default() -> Self {
        Self::new()
    }
}

/// 사용자에게 허락을 물을 때 보여 줄 viewer 정보.
#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub viewer_name: String,
    pub viewer_key_fingerprint: String,
}

pub enum ServeOutcome {
    Session(HostSession),
    Failed(AttemptFailure),
    /// PAKE 가 끝나기 전에 viewer 가 떠남.
    ViewerLeft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttemptFailure {
    /// 추측 제한 대기 중이라 PAKE 를 하지 않음.
    WaitRequired,
    /// Reply 를 보낸 뒤 viewer 가 확인 MAC 없이 떠나거나 시간이 지남.
    NoConfirm,
    WrongCode,
    Malformed,
    Timeout(Stage),
    Denied,
    VersionMismatch,
    BadIdentity,
    Transport,
}

/// signaling 자체가 끊긴 경우만. viewer 쪽 문제는 `ServeOutcome::Failed`.
#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error("signaling error: {0}")]
    Link(#[from] LinkError),
    #[error("transport error: {0}")]
    Transport(#[from] TransportError),
}

/// 허락받은 viewer 와의 연결.
pub struct HostSession {
    pub viewer: PeerIdentity,
    ch: Channel,
}

impl HostSession {
    /// Ping 에 Pong 으로 답한다. 연결이 닫히거나 `for_at_most` 가 지나면 끝난다.
    pub fn serve_pings(&mut self, for_at_most: Duration) -> Result<(), HostError> {
        let deadline = Instant::now() + for_at_most;
        loop {
            match self.ch.next(deadline)? {
                Next::Msg(Control::Ping { seq }) => self.ch.send(&Control::Pong { seq })?,
                Next::Msg(_) => {}
                Next::Malformed => {
                    // 해독할 수 없는 메시지를 보내는 상대와는 연결을 끊는다.
                    self.ch.close();
                    return Ok(());
                }
                Next::Closed | Next::TimedOut => return Ok(()),
            }
        }
    }
}

/// `ViewerJoined` 를 기다려 viewer 하나를 끝까지 처리하고 돌아온다.
/// `approve` 는 막히지 않는 질문 함수다: `None` 은 아직 답 없음, `Some(true/false)` 는 답.
/// 허락을 기다리는 동안 host 는 peer 를 계속 poll 하며 약 50ms 마다 `approve` 를 부르고,
/// `timeouts.approval` 이 지나면 `NoResponse` 로 거절한다.
pub fn serve_next<L: Link<ServerToHost, HostToServer>>(
    link: &mut L,
    cfg: &HostConfig,
    state: &mut HostState,
    approve: &mut dyn FnMut(&ApprovalRequest) -> Option<bool>,
) -> Result<ServeOutcome, HostError> {
    // 앞 viewer 가 남긴 ViewerLeft, Relay 는 버린다.
    while link.recv(cfg.timeouts.signaling_step)? != Some(ServerToHost::ViewerJoined) {}

    attempt(link, cfg, state, approve)
}

fn attempt<L: Link<ServerToHost, HostToServer>>(
    link: &mut L,
    cfg: &HostConfig,
    state: &mut HostState,
    approve: &mut dyn FnMut(&ApprovalRequest) -> Option<bool>,
) -> Result<ServeOutcome, HostError> {
    use AttemptFailure as F;
    let t = &cfg.timeouts;

    if let Err(wait) = state.limiter.check(Instant::now()) {
        // 올림: 남은 시간이 1ms 미만이어도 0 을 보내지 않는다.
        let ms = u64::try_from(wait.as_nanos().div_ceil(1_000_000)).unwrap_or(u64::MAX);
        send_relay(link, &PakeMsg::RetryAfter { ms })?;
        return kicked(link, F::WaitRequired);
    }

    let pa = match recv_relay(link, t.signaling_step)? {
        Relay::Msg(PakeMsg::Start { pa }) => pa,
        Relay::Msg(_) | Relay::Malformed => return reject_relay(link, F::Malformed),
        Relay::Left => return Ok(ServeOutcome::ViewerLeft),
        Relay::TimedOut => return kicked(link, F::Timeout(Stage::Pake)),
    };
    let Ok((pake, pb, mac_b)) = HostPake::respond(&state.code, &cfg.host_id, &pa) else {
        return reject_relay(link, F::Malformed);
    };

    // Reply 를 보내기 전에 실패로 먼저 기록한다. 틀린 코드의 viewer 는 mac_b 로 알아채고
    // 확인 MAC 없이 떠날 수 있으므로, 확인 때 기록하면 대기 없이 추측할 수 있다.
    // 코드를 바꿔도 이번 시도는 이미 계산한 옛 코드로 계속한다.
    if state.limiter.record_failure(Instant::now()).rotate_code {
        state.code = OneTimeCode::generate();
    }
    send_relay(link, &PakeMsg::Reply { pb, mac_b })?;

    let (mac_a, sealed_offer) = match recv_relay(link, t.signaling_step)? {
        Relay::Msg(PakeMsg::Confirm { mac_a, sealed_offer }) => (mac_a, sealed_offer),
        Relay::Msg(_) | Relay::Malformed => return reject_relay(link, F::Malformed),
        // 이미 떠난 viewer 에게는 Kick 하지 않는다. 그 사이 들어온 다음 viewer 가 대신 쫓겨난다.
        Relay::Left => return Ok(ServeOutcome::Failed(F::NoConfirm)),
        Relay::TimedOut => return kicked(link, F::NoConfirm),
    };
    let Ok(sk) = pake.confirm(&mac_a) else {
        return reject_relay(link, F::WrongCode);
    };
    // 코드는 맞았다. 뒤에서 봉인 열기가 실패해도 초기화는 유지한다.
    state.limiter.record_success();

    let (mut sender, mut receiver) = sk.host_side();
    let Some(offer) = receiver.open(&sealed_offer).ok().and_then(|b| String::from_utf8(b).ok()) else {
        return reject_relay(link, F::Malformed);
    };
    let Ok(peer) = Peer::new(&cfg.bind_ips) else {
        return kicked(link, F::Transport);
    };
    // 이 뒤로 실패해 돌아가면 `ch` 가 drop 되며 peer 를 닫는다.
    let mut ch = Channel::new(peer);
    let Ok(answer) = ch.peer().accept_offer(&offer) else {
        return reject_relay(link, F::Malformed);
    };
    send_relay(link, &PakeMsg::Answer { sealed_answer: sender.seal(answer.as_bytes()) })?;

    match handshake(ch, cfg, state, &sk, approve) {
        ServeOutcome::Failed(failure) => kicked(link, failure),
        other => Ok(other),
    }
}

/// data channel 이 열린 뒤 Hello 를 주고받고 허락을 묻는다.
fn handshake(
    mut ch: Channel,
    cfg: &HostConfig,
    state: &mut HostState,
    sk: &SessionKeys,
    approve: &mut dyn FnMut(&ApprovalRequest) -> Option<bool>,
) -> ServeOutcome {
    use AttemptFailure as F;
    let t = &cfg.timeouts;
    let fail = ServeOutcome::Failed;

    match ch.wait_open(Instant::now() + t.connect) {
        Ok(Opened::Ready) => {}
        Ok(Opened::TimedOut) => return fail(F::Timeout(Stage::Connect)),
        Ok(Opened::Closed) | Err(_) => return fail(F::Transport),
    }

    let local = ch.peer().local_fingerprint();
    let Some(remote) = ch.peer().remote_fingerprint() else {
        return fail(F::BadIdentity);
    };
    let mut hello = cfg.keys.make_hello(Role::Host, &sk.hello_binding, &local, &remote);
    hello.protocol_version = cfg.protocol_version;
    if ch.send(&Control::Hello(hello)).is_err() {
        return fail(F::Transport);
    }

    let viewer_hello = match ch.next(Instant::now() + t.signaling_step) {
        Ok(Next::Msg(Control::Hello(h))) => h,
        Ok(Next::Msg(_) | Next::Malformed) => return fail(F::Malformed),
        Ok(Next::TimedOut) => return fail(F::Timeout(Stage::Hello)),
        Ok(Next::Closed) | Err(_) => return fail(F::Transport),
    };
    let Ok(viewer) = verify_hello(&viewer_hello, Role::Viewer, &sk.hello_binding, &remote, &local) else {
        return reject_decision(ch, RejectReason::BadIdentity, F::BadIdentity, t.signaling_step);
    };
    if viewer_hello.protocol_version != cfg.protocol_version {
        let reason = RejectReason::VersionMismatch { host_version: cfg.protocol_version };
        return reject_decision(ch, reason, F::VersionMismatch, t.signaling_step);
    }

    let req = ApprovalRequest {
        viewer_name: viewer.name.clone(),
        viewer_key_fingerprint: key_fingerprint(&viewer.device_key),
    };
    match wait_approval(&mut ch, &req, approve, t.approval) {
        Approval::Granted => {}
        Approval::Denied => return reject_decision(ch, RejectReason::Denied, F::Denied, t.signaling_step),
        Approval::NoResponse => return reject_decision(ch, RejectReason::NoResponse, F::Denied, t.signaling_step),
        Approval::Failed(f) => return fail(f),
    }
    if ch.send(&Control::Decision(Decision::Accepted)).is_err() {
        return fail(F::Transport);
    }
    state.code = OneTimeCode::generate();
    ServeOutcome::Session(HostSession { viewer, ch })
}

/// 허락 질문 사이에 peer 를 poll 하는 간격. 이 동안에도 STUN consent 에 답해야 ICE 가 유지된다.
const APPROVAL_POLL: Duration = Duration::from_millis(50);

enum Approval {
    Granted,
    Denied,
    NoResponse,
    Failed(AttemptFailure),
}

/// peer 를 계속 poll 하면서 `approve` 가 답할 때까지, 최대 `limit` 동안 기다린다.
/// 이 단계의 viewer 는 Decision 을 기다리기만 하므로, 무엇이든 보내면 규칙 위반으로 끊는다.
fn wait_approval(
    ch: &mut Channel,
    req: &ApprovalRequest,
    approve: &mut dyn FnMut(&ApprovalRequest) -> Option<bool>,
    limit: Duration,
) -> Approval {
    let deadline = Instant::now() + limit;
    loop {
        match approve(req) {
            Some(true) => return Approval::Granted,
            Some(false) => return Approval::Denied,
            None => {}
        }
        let now = Instant::now();
        if now >= deadline {
            return Approval::NoResponse;
        }
        match ch.next((now + APPROVAL_POLL).min(deadline)) {
            Ok(Next::TimedOut) => {}
            Ok(Next::Msg(_) | Next::Malformed) => return Approval::Failed(AttemptFailure::Malformed),
            Ok(Next::Closed) | Err(_) => return Approval::Failed(AttemptFailure::Transport),
        }
    }
}

/// signaling 에 남아 있는 viewer 를 내보내고 실패로 끝낸다.
fn kicked<L: Link<ServerToHost, HostToServer>>(
    link: &mut L,
    failure: AttemptFailure,
) -> Result<ServeOutcome, HostError> {
    link.send(&HostToServer::Kick)?;
    Ok(ServeOutcome::Failed(failure))
}

/// signaling 으로 `Rejected` 를 알린 뒤 Kick 한다.
fn reject_relay<L: Link<ServerToHost, HostToServer>>(
    link: &mut L,
    failure: AttemptFailure,
) -> Result<ServeOutcome, HostError> {
    send_relay(link, &PakeMsg::Rejected)?;
    kicked(link, failure)
}

/// data channel 로 거절을 알리고, viewer 가 받고 peer 를 닫을 때까지(최대 `linger`) poll 을 이어 간다.
/// 곧바로 닫으면 SCTP 가 Decision 을 내보내거나 재전송하기 전에 peer 가 사라질 수 있다.
fn reject_decision(mut ch: Channel, reason: RejectReason, failure: AttemptFailure, linger: Duration) -> ServeOutcome {
    if ch.send(&Control::Decision(Decision::Rejected(reason))).is_ok() {
        let deadline = Instant::now() + linger;
        while let Ok(Next::Msg(_) | Next::Malformed) = ch.next(deadline) {}
    }
    ServeOutcome::Failed(failure)
}

fn send_relay<L: Link<ServerToHost, HostToServer>>(link: &mut L, msg: &PakeMsg) -> Result<(), LinkError> {
    link.send(&HostToServer::Relay { data: hex::encode(pake_msg::encode(msg)) })
}

enum Relay {
    Msg(PakeMsg),
    Malformed,
    Left,
    TimedOut,
}

fn recv_relay<L: Link<ServerToHost, HostToServer>>(link: &mut L, timeout: Duration) -> Result<Relay, LinkError> {
    Ok(match link.recv(timeout)? {
        Some(ServerToHost::Relay { data }) => match hex::decode(&data).ok().and_then(|b| pake_msg::decode(&b).ok()) {
            Some(msg) => Relay::Msg(msg),
            None => Relay::Malformed,
        },
        Some(ServerToHost::ViewerLeft) => Relay::Left,
        // 시도 중에 올 수 없는 signaling 메시지.
        Some(_) => Relay::Malformed,
        None => Relay::TimedOut,
    })
}

