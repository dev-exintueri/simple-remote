//! signaling Worker 없이 `Link` 를 테스트하기 위한 메모리 가짜.
//! 실제 서버(`signaling/src/host-room.ts`)처럼 host 하나에 열린 viewer 하나만 허용하고,
//! viewer 번호로 늦게 도착한 relay/kick 이 다음 viewer 에 잘못 닿지 않게 막는다.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use protocol::signaling::{HostMode, HostToServer, JoinKind, ServerToHost, ServerToViewer, ViewerToServer};

use crate::link::{Link, LinkError};

/// viewer 에게 보내는 내부 봉투. `Closed` 는 큐에 남은 메시지를 다 내보낸 뒤에 온다.
enum ViewerEnvelope {
    Msg(ServerToViewer),
    Closed(u16),
}

struct ViewerSlot {
    n: u64,
    tx: mpsc::Sender<ViewerEnvelope>,
}

struct Inner {
    host_tx: mpsc::Sender<ServerToHost>,
    host_relays: Vec<Vec<u8>>,
    host_present: bool,
    mode: HostMode,
    viewer: Option<ViewerSlot>,
    /// 이 room 이 지금까지 받아들인 viewer 수 + 1. 1 부터 시작.
    next_n: u64,
}

/// host 하나짜리 fake signaling hub. `Clone` 은 같은 hub 를 공유한다(내부는 `Arc`).
#[derive(Clone)]
pub struct FakeHub {
    inner: Arc<Mutex<Inner>>,
}

impl FakeHub {
    pub fn new() -> (FakeHub, FakeHostLink) {
        let (host_tx, host_rx) = mpsc::channel();
        let inner = Arc::new(Mutex::new(Inner {
            host_tx,
            host_relays: Vec::new(),
            host_present: true,
            // Worker 와 같게: host 가 `Mode` 를 보내기 전 기본 모드는 reconnect.
            mode: HostMode::Reconnect,
            viewer: None,
            next_n: 1,
        }));
        let hub = FakeHub { inner: inner.clone() };
        let host = FakeHostLink { inner, rx: host_rx, closed_code: None };
        (hub, host)
    }

    /// 새 viewer 를 만든다. Worker 와 같게 host 가 없거나 현재 모드가 `kind` 를 받지 않으면
    /// `Http{404}`, 이미 열린 viewer 가 있으면 `Http{409}`.
    pub fn viewer(&self, kind: JoinKind) -> Result<FakeViewerLink, LinkError> {
        let (tx, rx) = mpsc::channel();
        let mut inner = self.inner.lock().expect("fake signal mutex poisoned");

        if !inner.host_present {
            return Err(LinkError::Http { status: 404 });
        }
        let allowed = match kind {
            JoinKind::New => inner.mode == HostMode::New,
            JoinKind::Reconnect => true,
        };
        if !allowed {
            return Err(LinkError::Http { status: 404 });
        }
        if inner.viewer.is_some() {
            return Err(LinkError::Http { status: 409 });
        }

        let n = inner.next_n;
        inner.next_n += 1;
        inner.viewer = Some(ViewerSlot { n, tx: tx.clone() });

        let _ = inner.host_tx.send(ServerToHost::ViewerJoined { n, kind });
        let _ = tx.send(ViewerEnvelope::Msg(ServerToViewer::Joined));

        Ok(FakeViewerLink { inner: self.inner.clone(), rx, n, closed_code: None })
    }

    /// host 가 `HostToServer::Relay` 로 보낸 data 를 hex 해독해 보낸 순서대로 돌려준다.
    pub fn host_relays(&self) -> Vec<Vec<u8>> {
        self.inner.lock().expect("fake signal mutex poisoned").host_relays.clone()
    }
}

pub struct FakeHostLink {
    inner: Arc<Mutex<Inner>>,
    rx: mpsc::Receiver<ServerToHost>,
    closed_code: Option<Option<u16>>,
}

impl Link<ServerToHost, HostToServer> for FakeHostLink {
    fn send(&mut self, msg: &HostToServer) -> Result<(), LinkError> {
        if let Some(code) = self.closed_code {
            return Err(LinkError::Closed { code });
        }

        match msg {
            // fake 는 등록/인증을 모델링하지 않으므로 무시한다.
            HostToServer::Auth { .. } => {}
            HostToServer::Mode { mode } => {
                let mut inner = self.inner.lock().expect("fake signal mutex poisoned");
                inner.mode = *mode;
            }
            HostToServer::Relay { n, data } => {
                let bytes = hex::decode(data).expect("host 는 항상 유효한 hex 를 보낸다");
                let mut inner = self.inner.lock().expect("fake signal mutex poisoned");
                inner.host_relays.push(bytes);
                // 열린 viewer 의 번호와 다르면(없거나 이미 다른 viewer 로 바뀌었으면) 버린다.
                if let Some(viewer) = &inner.viewer {
                    if viewer.n == *n {
                        let _ = viewer.tx.send(ViewerEnvelope::Msg(ServerToViewer::Relay { data: data.clone() }));
                    }
                }
            }
            HostToServer::Kick { n } => {
                let mut inner = self.inner.lock().expect("fake signal mutex poisoned");
                let matches = inner.viewer.as_ref().map(|v| v.n) == Some(*n);
                if matches {
                    if let Some(viewer) = inner.viewer.take() {
                        let _ = viewer.tx.send(ViewerEnvelope::Closed(4001));
                    }
                }
                // 번호가 다르면(이미 떠났거나 다음 viewer 로 바뀌었으면) 조용히 버린다.
            }
        }
        Ok(())
    }

    fn recv(&mut self, timeout: Duration) -> Result<Option<ServerToHost>, LinkError> {
        if let Some(code) = self.closed_code {
            return Err(LinkError::Closed { code });
        }

        match self.rx.recv_timeout(timeout) {
            Ok(msg) => Ok(Some(msg)),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.closed_code = Some(None);
                Err(LinkError::Closed { code: None })
            }
        }
    }
}

impl Drop for FakeHostLink {
    fn drop(&mut self) {
        let mut inner = self.inner.lock().expect("fake signal mutex poisoned");
        inner.host_present = false;
        if let Some(viewer) = inner.viewer.take() {
            let _ = viewer.tx.send(ViewerEnvelope::Msg(ServerToViewer::HostLeft));
            let _ = viewer.tx.send(ViewerEnvelope::Closed(4002));
        }
    }
}

pub struct FakeViewerLink {
    inner: Arc<Mutex<Inner>>,
    rx: mpsc::Receiver<ViewerEnvelope>,
    n: u64,
    closed_code: Option<Option<u16>>,
}

impl FakeViewerLink {
    fn is_current(&self, inner: &Inner) -> bool {
        inner.viewer.as_ref().map(|v| v.n) == Some(self.n)
    }
}

impl Link<ServerToViewer, ViewerToServer> for FakeViewerLink {
    fn send(&mut self, msg: &ViewerToServer) -> Result<(), LinkError> {
        if let Some(code) = self.closed_code {
            return Err(LinkError::Closed { code });
        }

        let inner = self.inner.lock().expect("fake signal mutex poisoned");
        if !self.is_current(&inner) {
            return Err(LinkError::Closed { code: Some(4001) });
        }

        let ViewerToServer::Relay { data } = msg;
        let _ = inner.host_tx.send(ServerToHost::Relay { n: self.n, data: data.clone() });
        Ok(())
    }

    fn recv(&mut self, timeout: Duration) -> Result<Option<ServerToViewer>, LinkError> {
        if let Some(code) = self.closed_code {
            return Err(LinkError::Closed { code });
        }

        match self.rx.recv_timeout(timeout) {
            Ok(ViewerEnvelope::Msg(msg)) => Ok(Some(msg)),
            Ok(ViewerEnvelope::Closed(code)) => {
                self.closed_code = Some(Some(code));
                Err(LinkError::Closed { code: Some(code) })
            }
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.closed_code = Some(None);
                Err(LinkError::Closed { code: None })
            }
        }
    }
}

impl Drop for FakeViewerLink {
    fn drop(&mut self) {
        let mut inner = self.inner.lock().expect("fake signal mutex poisoned");
        // 이미 다음 viewer 로 바뀌었거나 kick 됐으면(inner.viewer 가 이 viewer 를 가리키지 않으면)
        // ViewerLeft 를 보내지 않는다. 아직 현재 viewer 인 채로 드롭됐을 때만 보낸다.
        if self.is_current(&inner) {
            inner.viewer = None;
            let _ = inner.host_tx.send(ServerToHost::ViewerLeft { n: self.n });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewer_join_and_relay() {
        let (hub, mut host) = FakeHub::new();
        host.send(&HostToServer::Mode { mode: HostMode::New }).unwrap();
        let mut viewer = hub.viewer(JoinKind::New).unwrap();

        assert_eq!(
            host.recv(Duration::from_secs(1)).unwrap(),
            Some(ServerToHost::ViewerJoined { n: 1, kind: JoinKind::New })
        );
        assert_eq!(viewer.recv(Duration::from_secs(1)).unwrap(), Some(ServerToViewer::Joined));

        host.send(&HostToServer::Relay { n: 1, data: "abcd".into() }).unwrap();
        assert_eq!(
            viewer.recv(Duration::from_secs(1)).unwrap(),
            Some(ServerToViewer::Relay { data: "abcd".into() })
        );

        viewer.send(&ViewerToServer::Relay { data: "ef01".into() }).unwrap();
        assert_eq!(
            host.recv(Duration::from_secs(1)).unwrap(),
            Some(ServerToHost::Relay { n: 1, data: "ef01".into() })
        );

        assert_eq!(hub.host_relays(), vec![vec![0xab, 0xcd]]);
    }

    #[test]
    fn relay_with_no_viewer_is_dropped() {
        let (hub, mut host) = FakeHub::new();
        host.send(&HostToServer::Relay { n: 1, data: "aa".into() }).unwrap();
        assert_eq!(hub.host_relays(), vec![vec![0xaa]]);
        // 버려졌으니 아무도 못 받지만, 기록은 남는다 — 여기서는 그냥 panic 하지 않는지만 본다.
    }

    #[test]
    fn kick_closes_viewer() {
        let (hub, mut host) = FakeHub::new();
        host.send(&HostToServer::Mode { mode: HostMode::New }).unwrap();
        let mut viewer = hub.viewer(JoinKind::New).unwrap();
        assert_eq!(
            host.recv(Duration::from_secs(1)).unwrap(),
            Some(ServerToHost::ViewerJoined { n: 1, kind: JoinKind::New })
        );
        assert_eq!(viewer.recv(Duration::from_secs(1)).unwrap(), Some(ServerToViewer::Joined));

        host.send(&HostToServer::Relay { n: 1, data: "aa".into() }).unwrap();
        host.send(&HostToServer::Kick { n: 1 }).unwrap();

        assert_eq!(
            viewer.recv(Duration::from_secs(1)).unwrap(),
            Some(ServerToViewer::Relay { data: "aa".into() })
        );
        assert!(matches!(
            viewer.recv(Duration::from_secs(1)),
            Err(LinkError::Closed { code: Some(4001) })
        ));

        // host 는 자기가 보낸 kick 에 대해 ViewerLeft 를 받지 않는다.
        assert!(host.recv(Duration::from_millis(100)).unwrap().is_none());
    }

    #[test]
    fn drops_notify_other_side() {
        let (hub, mut host) = FakeHub::new();
        host.send(&HostToServer::Mode { mode: HostMode::New }).unwrap();
        let viewer = hub.viewer(JoinKind::New).unwrap();
        assert_eq!(
            host.recv(Duration::from_secs(1)).unwrap(),
            Some(ServerToHost::ViewerJoined { n: 1, kind: JoinKind::New })
        );
        drop(viewer);
        assert_eq!(host.recv(Duration::from_secs(1)).unwrap(), Some(ServerToHost::ViewerLeft { n: 1 }));

        let (hub2, host2) = FakeHub::new();
        let mut viewer2 = hub2.viewer(JoinKind::Reconnect).unwrap();
        assert_eq!(viewer2.recv(Duration::from_secs(1)).unwrap(), Some(ServerToViewer::Joined));
        drop(host2);
        assert_eq!(viewer2.recv(Duration::from_secs(1)).unwrap(), Some(ServerToViewer::HostLeft));
        assert!(matches!(
            viewer2.recv(Duration::from_secs(1)),
            Err(LinkError::Closed { code: Some(4002) })
        ));
    }

    #[test]
    fn fake_mode_gates_join() {
        let (hub, _host) = FakeHub::new();
        // 기본 모드는 reconnect: new 는 거절되고 reconnect 는 받아들여진다.
        assert!(matches!(hub.viewer(JoinKind::New), Err(LinkError::Http { status: 404 })));
        assert!(hub.viewer(JoinKind::Reconnect).is_ok());
    }

    #[test]
    fn fake_second_viewer_is_busy() {
        let (hub, mut host) = FakeHub::new();
        host.send(&HostToServer::Mode { mode: HostMode::New }).unwrap();
        let _v1 = hub.viewer(JoinKind::New).unwrap();
        assert_eq!(
            host.recv(Duration::from_secs(1)).unwrap(),
            Some(ServerToHost::ViewerJoined { n: 1, kind: JoinKind::New })
        );
        assert!(matches!(hub.viewer(JoinKind::New), Err(LinkError::Http { status: 409 })));
    }

    #[test]
    fn fake_ignores_stale_kick() {
        let (hub, mut host) = FakeHub::new();
        host.send(&HostToServer::Mode { mode: HostMode::New }).unwrap();
        let v1 = hub.viewer(JoinKind::New).unwrap();
        assert_eq!(
            host.recv(Duration::from_secs(1)).unwrap(),
            Some(ServerToHost::ViewerJoined { n: 1, kind: JoinKind::New })
        );
        drop(v1);
        assert_eq!(host.recv(Duration::from_secs(1)).unwrap(), Some(ServerToHost::ViewerLeft { n: 1 }));

        let mut v2 = hub.viewer(JoinKind::New).unwrap();
        assert_eq!(
            host.recv(Duration::from_secs(1)).unwrap(),
            Some(ServerToHost::ViewerJoined { n: 2, kind: JoinKind::New })
        );
        assert_eq!(v2.recv(Duration::from_secs(1)).unwrap(), Some(ServerToViewer::Joined));

        // 이제는 없는 viewer 1 을 향한 늦은 kick: 번호가 달라 v2 에는 아무 영향이 없다.
        host.send(&HostToServer::Kick { n: 1 }).unwrap();
        assert_eq!(v2.recv(Duration::from_millis(100)).unwrap(), None);
    }
}
