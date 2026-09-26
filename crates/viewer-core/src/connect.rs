//! viewer 쪽 첫 연결 흐름: signaling 위의 PAKE, 봉인한 SDP 교환, data channel 위의 Hello 와 허락.

use std::collections::VecDeque;
use std::net::IpAddr;
use std::time::{Duration, Instant};

use auth::{AuthError, DeviceKeys, OneTimeCode, PeerIdentity, Role, SessionKeys, ViewerPake, verify_hello};
use protocol::control::{self, Control, Decision, RejectReason};
use protocol::pake_msg::{self, PakeMsg};
use protocol::signaling::{ServerToViewer, ViewerToServer};
use protocol::{PROTOCOL_VERSION, Stage, Timeouts};
use transport::{Link, LinkError, Peer, PeerEvent, TransportError};

/// 버전이 더 낮은 쪽.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Viewer,
    Host,
}

#[derive(Debug, thiserror::Error)]
pub enum ConnectError {
    #[error("wrong code")]
    WrongCode,
    #[error("host asks to retry after {0:?}")]
    RetryAfter(Duration),
    #[error("host is not waiting")]
    HostNotWaiting,
    #[error("host is busy with another viewer")]
    Busy,
    #[error("rate limited by signaling server")]
    RateLimited,
    #[error("host left")]
    HostLeft,
    #[error("host rejected: {0:?}")]
    Rejected(RejectReason),
    #[error("protocol version mismatch (ours {ours}, theirs {theirs}, older {older:?})")]
    VersionMismatch { ours: u32, theirs: u32, older: Side },
    #[error("timed out at {0:?}")]
    Timeout(Stage),
    #[error("security check failed: {0}")]
    Security(&'static str),
    #[error("signaling error: {0}")]
    Link(LinkError),
    #[error("transport error: {0}")]
    Transport(#[from] TransportError),
}

impl ConnectError {
    /// signaling 서버에 들어가는 단계(WebSocket upgrade)의 오류를 사용자에게 보일 오류로 바꾼다.
    pub fn from_join_error(e: LinkError) -> ConnectError {
        match e {
            LinkError::Http { status: 404 } => ConnectError::HostNotWaiting,
            LinkError::Http { status: 409 } => ConnectError::Busy,
            LinkError::Http { status: 429 } => ConnectError::RateLimited,
            other => ConnectError::Link(other),
        }
    }
}

/// 연결이 끝난 뒤의 오류: link 가 닫혔으면 host 가 떠난 것으로 본다.
fn from_link(e: LinkError) -> ConnectError {
    match e {
        LinkError::Closed { .. } => ConnectError::HostLeft,
        other => ConnectError::Link(other),
    }
}

/// host 가 받아들인 연결.
pub struct ViewerSession {
    pub host: PeerIdentity,
    ch: Channel,
    next_seq: u64,
}

impl std::fmt::Debug for ViewerSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ViewerSession").field("host", &self.host).finish_non_exhaustive()
    }
}

impl ViewerSession {
    /// Ping 을 보내고 같은 seq 의 Pong 까지 걸린 시간(RTT)을 돌려준다.
    pub fn ping(&mut self, timeout: Duration) -> Result<Duration, ConnectError> {
        let seq = self.next_seq;
        self.next_seq += 1;
        let start = Instant::now();
        let deadline = start + timeout;
        self.ch.send(&Control::Ping { seq })?;
        loop {
            match self.ch.next_control(deadline, Stage::Ping)? {
                Control::Pong { seq: got } if got == seq => return Ok(start.elapsed()),
                // 앞서 시간이 지난 Ping 의 늦은 Pong 등은 건너뛴다.
                _ => continue,
            }
        }
    }
}

/// host `host_id` 에 `code` 로 연결한다. `link` 는 이미 `Joined` 를 기다리는 상태여야 한다.
/// 어떤 결과든 돌아올 때 `link` 는 닫힌다 (성공하면 signaling 자리를 비우려고, 실패하면 정리하려고).
pub fn connect<L: Link<ServerToViewer, ViewerToServer>>(
    mut link: L,
    host_id: &str,
    code: &OneTimeCode,
    keys: &DeviceKeys,
    bind_ips: &[IpAddr],
    t: &Timeouts,
) -> Result<ViewerSession, ConnectError> {
    match link.recv(t.signaling_step).map_err(from_link)? {
        Some(ServerToViewer::Joined) => {}
        Some(ServerToViewer::HostLeft) => return Err(ConnectError::HostLeft),
        Some(ServerToViewer::Relay { .. }) => return Err(ConnectError::Security("unexpected relay")),
        None => return Err(ConnectError::Timeout(Stage::Join)),
    }

    let (pake, pa) = ViewerPake::start(code, host_id).map_err(|_| ConnectError::Security("pake start"))?;
    send_relay(&mut link, &PakeMsg::Start { pa })?;

    let (session_keys, mac_a) = match recv_relay(&mut link, t.signaling_step, Stage::Pake)? {
        PakeMsg::Reply { pb, mac_b } => pake.finish(&pb, &mac_b).map_err(|e| match e {
            // 주소를 보내기 전에 끝낸다. link 는 돌아가며 닫힌다.
            AuthError::WrongCode => ConnectError::WrongCode,
            _ => ConnectError::Security("malformed relay"),
        })?,
        PakeMsg::RetryAfter { ms } => return Err(ConnectError::RetryAfter(Duration::from_millis(ms))),
        _ => return Err(ConnectError::Security("unexpected relay")),
    };

    // 실패로 돌아가면 `ch` 가 drop 되며 peer 를 닫는다.
    let mut ch = Channel::new(Peer::new(bind_ips)?);
    let host = handshake(&mut link, &mut ch, &session_keys, mac_a, keys, t)?;
    drop(link);
    Ok(ViewerSession { host, ch, next_seq: 0 })
}

fn handshake<L: Link<ServerToViewer, ViewerToServer>>(
    link: &mut L,
    ch: &mut Channel,
    sk: &SessionKeys,
    mac_a: Vec<u8>,
    keys: &DeviceKeys,
    t: &Timeouts,
) -> Result<PeerIdentity, ConnectError> {
    let (offer, pending) = ch.peer.create_offer()?;
    let (mut sender, mut receiver) = sk.viewer_side();
    let sealed_offer = sender.seal(offer.as_bytes());
    send_relay(link, &PakeMsg::Confirm { mac_a, sealed_offer })?;

    let sealed_answer = match recv_relay(link, t.signaling_step, Stage::Answer)? {
        PakeMsg::Answer { sealed_answer } => sealed_answer,
        PakeMsg::Rejected => return Err(ConnectError::Security("host rejected confirm")),
        _ => return Err(ConnectError::Security("unexpected relay")),
    };
    let answer = receiver
        .open(&sealed_answer)
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .ok_or(ConnectError::Security("sealed answer"))?;
    ch.peer.accept_answer(pending, &answer)?;

    match ch.wait_open(Instant::now() + t.connect)? {
        Opened::Ready => {}
        Opened::Closed => return Err(ConnectError::HostLeft),
        Opened::TimedOut => return Err(ConnectError::Timeout(Stage::Connect)),
    }

    let local = ch.peer.local_fingerprint();
    let remote = ch.peer.remote_fingerprint().ok_or(ConnectError::Security("no remote fingerprint"))?;
    ch.send(&Control::Hello(keys.make_hello(Role::Viewer, &sk.hello_binding, &local, &remote)))?;

    let host = match ch.next_control(Instant::now() + t.signaling_step, Stage::Hello)? {
        Control::Hello(h) => verify_hello(&h, Role::Host, &sk.hello_binding, &remote, &local)
            .map_err(|_| ConnectError::Security("host identity"))?,
        _ => return Err(ConnectError::Security("unexpected control")),
    };

    match ch.next_control(Instant::now() + t.approval + t.signaling_step, Stage::Decision)? {
        Control::Decision(Decision::Accepted) => Ok(host),
        Control::Decision(Decision::Rejected(RejectReason::VersionMismatch { host_version })) => {
            Err(ConnectError::VersionMismatch {
                ours: PROTOCOL_VERSION,
                theirs: host_version,
                older: if PROTOCOL_VERSION < host_version { Side::Viewer } else { Side::Host },
            })
        }
        Control::Decision(Decision::Rejected(reason)) => Err(ConnectError::Rejected(reason)),
        _ => Err(ConnectError::Security("unexpected control")),
    }
}

/// 보낸 뒤에는 항상 `recv_relay` 가 이어진다. link 가 이미 닫혔다는 오류는 여기서 내지 않고
/// 그 `recv_relay` 에 맡긴다: host 가 닫기 전에 보낸 메시지(예: `RetryAfter` 뒤의 Kick)가
/// 아직 큐에 있으면 그것을 먼저 읽고, 없으면 거기서 `HostLeft` 가 된다.
fn send_relay<L: Link<ServerToViewer, ViewerToServer>>(link: &mut L, msg: &PakeMsg) -> Result<(), ConnectError> {
    match link.send(&ViewerToServer::Relay { data: hex::encode(pake_msg::encode(msg)) }) {
        Ok(()) | Err(LinkError::Closed { .. }) => Ok(()),
        Err(e) => Err(ConnectError::Link(e)),
    }
}

fn recv_relay<L: Link<ServerToViewer, ViewerToServer>>(
    link: &mut L,
    timeout: Duration,
    stage: Stage,
) -> Result<PakeMsg, ConnectError> {
    match link.recv(timeout).map_err(from_link)? {
        Some(ServerToViewer::Relay { data }) => hex::decode(&data)
            .ok()
            .and_then(|b| pake_msg::decode(&b).ok())
            .ok_or(ConnectError::Security("malformed relay")),
        Some(ServerToViewer::HostLeft) => Err(ConnectError::HostLeft),
        Some(ServerToViewer::Joined) => Err(ConnectError::Security("unexpected signaling")),
        None => Err(ConnectError::Timeout(stage)),
    }
}

enum Opened {
    Ready,
    Closed,
    TimedOut,
}

enum Next {
    Data(Vec<u8>),
    Closed,
    TimedOut,
}

/// `Peer` 와, 기다리는 동안 먼저 도착한 data 를 담아 두는 큐. drop 되면 peer 를 닫는다.
struct Channel {
    peer: Peer,
    pending: VecDeque<Vec<u8>>,
    closed: bool,
}

impl Channel {
    fn new(peer: Peer) -> Channel {
        Channel { peer, pending: VecDeque::new(), closed: false }
    }

    fn absorb(&mut self, events: Vec<PeerEvent>) -> bool {
        let mut opened = false;
        for ev in events {
            match ev {
                PeerEvent::ChannelOpen => opened = true,
                PeerEvent::Data(d) => self.pending.push_back(d),
                PeerEvent::Closed => self.closed = true,
                PeerEvent::Connected => {}
            }
        }
        opened
    }

    fn wait_open(&mut self, deadline: Instant) -> Result<Opened, TransportError> {
        loop {
            if self.closed {
                return Ok(Opened::Closed);
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok(Opened::TimedOut);
            }
            let events = self.peer.poll(deadline - now)?;
            if self.absorb(events) {
                return Ok(Opened::Ready);
            }
        }
    }

    fn next(&mut self, deadline: Instant) -> Result<Next, TransportError> {
        loop {
            if let Some(d) = self.pending.pop_front() {
                return Ok(Next::Data(d));
            }
            if self.closed {
                return Ok(Next::Closed);
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok(Next::TimedOut);
            }
            let events = self.peer.poll(deadline - now)?;
            self.absorb(events);
        }
    }

    fn next_control(&mut self, deadline: Instant, stage: Stage) -> Result<Control, ConnectError> {
        match self.next(deadline)? {
            Next::Data(d) => control::decode(&d).map_err(|_| ConnectError::Security("malformed control")),
            Next::Closed => Err(ConnectError::HostLeft),
            Next::TimedOut => Err(ConnectError::Timeout(stage)),
        }
    }

    fn send(&mut self, msg: &Control) -> Result<(), TransportError> {
        self.peer.send(&control::encode(msg))
    }
}

impl Drop for Channel {
    fn drop(&mut self) {
        // close 는 SCTP shutdown 과 DTLS close_notify 를 준비만 하므로 한 번 내보내 상대가 빨리 알게 한다.
        self.peer.close();
        let _ = self.peer.poll(Duration::ZERO);
    }
}
