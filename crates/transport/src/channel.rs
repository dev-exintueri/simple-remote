//! viewer 와 host 가 함께 쓰는 control data channel 처리.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use protocol::control::{self, Control};

use crate::error::TransportError;
use crate::peer::{Peer, PeerEvent};

/// `Channel::wait_open` 의 결과.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opened {
    Ready,
    Closed,
    TimedOut,
}

/// `Channel::next` 의 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    Msg(Control),
    /// 해독할 수 없는 메시지. 받는 쪽은 연결을 끊어야 한다 (fail closed).
    Malformed,
    Closed,
    TimedOut,
}

/// `Peer` 와, 기다리는 동안 먼저 도착한 data 를 담아 두는 큐.
/// drop 되면 (이미 `close` 하지 않았으면) peer 를 닫는다.
pub struct Channel {
    peer: Peer,
    pending: VecDeque<Vec<u8>>,
    remote_closed: bool,
    local_closed: bool,
}

impl Channel {
    pub fn new(peer: Peer) -> Channel {
        Channel { peer, pending: VecDeque::new(), remote_closed: false, local_closed: false }
    }

    /// SDP 협상과 지문 조회를 위한 peer 접근.
    pub fn peer(&mut self) -> &mut Peer {
        &mut self.peer
    }

    /// `ChannelOpen` 이 오거나 닫히거나 `deadline` 이 지날 때까지 poll 한다.
    /// 그 사이 온 data 는 큐에 남겨 `next` 가 돌려준다.
    pub fn wait_open(&mut self, deadline: Instant) -> Result<Opened, TransportError> {
        loop {
            if self.remote_closed {
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

    /// 다음 control 메시지. 큐에 있는 것부터 돌려준다.
    pub fn next(&mut self, deadline: Instant) -> Result<Next, TransportError> {
        loop {
            if let Some(d) = self.pending.pop_front() {
                return Ok(match control::decode(&d) {
                    Ok(msg) => Next::Msg(msg),
                    Err(_) => Next::Malformed,
                });
            }
            if self.remote_closed {
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

    pub fn send(&mut self, msg: &Control) -> Result<(), TransportError> {
        self.peer.send(&control::encode(msg))
    }

    /// peer 를 닫고 닫기 출력을 한 번 내보낸다. `Peer::close` 는 SCTP shutdown 과
    /// DTLS close_notify 를 준비만 하므로, 내보내야 상대가 기다리지 않고 알게 된다.
    pub fn close(&mut self) {
        if self.local_closed {
            return;
        }
        self.local_closed = true;
        self.peer.close();
        let _ = self.peer.poll(Duration::ZERO);
    }

    fn absorb(&mut self, events: Vec<PeerEvent>) -> bool {
        let mut opened = false;
        for ev in events {
            match ev {
                PeerEvent::ChannelOpen => opened = true,
                PeerEvent::Data(d) => self.pending.push_back(d),
                PeerEvent::Closed => self.remote_closed = true,
                PeerEvent::Connected => {}
            }
        }
        opened
    }
}

impl Drop for Channel {
    fn drop(&mut self) {
        self.close();
    }
}
