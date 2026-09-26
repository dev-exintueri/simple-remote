//! signaling Worker 없이 `Link` 를 테스트하기 위한 메모리 가짜.
//! 실제 서버처럼 host 하나에 viewer 하나만 허용하고, 메시지 전달 순서를 지킨다.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use protocol::signaling::{HostToServer, ServerToHost, ServerToViewer, ViewerToServer};

use crate::link::{Link, LinkError};

/// viewer 에게 보내는 내부 봉투. `Closed` 는 큐에 남은 메시지를 다 내보낸 뒤에 온다.
enum ViewerEnvelope {
    Msg(ServerToViewer),
    Closed(u16),
}

struct ViewerSlot {
    generation: u64,
    tx: mpsc::Sender<ViewerEnvelope>,
}

struct Inner {
    host_tx: mpsc::Sender<ServerToHost>,
    host_relays: Vec<Vec<u8>>,
    viewer: Option<ViewerSlot>,
    next_generation: u64,
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
            viewer: None,
            next_generation: 0,
        }));
        let hub = FakeHub { inner: inner.clone() };
        let host = FakeHostLink { inner, rx: host_rx, closed_code: None };
        (hub, host)
    }

    /// 새 viewer 를 만든다. 이전 viewer 가 있었으면 `Closed{code:Some(4001)}` 을 받는다
    /// (한 명만 허용하는 서버를 모델링).
    pub fn viewer(&self) -> FakeViewerLink {
        let (tx, rx) = mpsc::channel();
        let mut inner = self.inner.lock().expect("fake signal mutex poisoned");

        let generation = inner.next_generation;
        inner.next_generation += 1;

        if let Some(old) = inner.viewer.take() {
            let _ = old.tx.send(ViewerEnvelope::Closed(4001));
        }
        inner.viewer = Some(ViewerSlot { generation, tx: tx.clone() });

        let _ = inner.host_tx.send(ServerToHost::ViewerJoined);
        let _ = tx.send(ViewerEnvelope::Msg(ServerToViewer::Joined));

        FakeViewerLink { inner: self.inner.clone(), rx, generation, closed_code: None }
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
            HostToServer::Relay { data } => {
                let bytes = hex::decode(data).expect("host 는 항상 유효한 hex 를 보낸다");
                let mut inner = self.inner.lock().expect("fake signal mutex poisoned");
                inner.host_relays.push(bytes);
                // viewer 가 없으면 실제 Worker 처럼 버린다.
                if let Some(viewer) = &inner.viewer {
                    let _ = viewer.tx.send(ViewerEnvelope::Msg(ServerToViewer::Relay { data: data.clone() }));
                }
            }
            HostToServer::Kick => {
                let mut inner = self.inner.lock().expect("fake signal mutex poisoned");
                if let Some(viewer) = inner.viewer.take() {
                    let _ = viewer.tx.send(ViewerEnvelope::Closed(4001));
                }
                // host 는 자기가 보낸 kick 에 대해 ViewerLeft 를 받지 않는다.
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
        if let Some(viewer) = inner.viewer.take() {
            let _ = viewer.tx.send(ViewerEnvelope::Msg(ServerToViewer::HostLeft));
            let _ = viewer.tx.send(ViewerEnvelope::Closed(4002));
        }
    }
}

pub struct FakeViewerLink {
    inner: Arc<Mutex<Inner>>,
    rx: mpsc::Receiver<ViewerEnvelope>,
    generation: u64,
    closed_code: Option<Option<u16>>,
}

impl FakeViewerLink {
    fn is_current(&self, inner: &Inner) -> bool {
        inner.viewer.as_ref().map(|v| v.generation) == Some(self.generation)
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
        let _ = inner.host_tx.send(ServerToHost::Relay { data: data.clone() });
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
        // 이미 새 viewer 로 교체됐거나(리플레이스) kick 됐으면(inner.viewer == None) ViewerLeft 를
        // 보내지 않는다. 아직 현재 viewer 인 채로 드롭됐을 때만 보낸다.
        if self.is_current(&inner) {
            inner.viewer = None;
            let _ = inner.host_tx.send(ServerToHost::ViewerLeft);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewer_join_and_relay() {
        let (hub, mut host) = FakeHub::new();
        let mut viewer = hub.viewer();

        assert_eq!(host.recv(Duration::from_secs(1)).unwrap(), Some(ServerToHost::ViewerJoined));
        assert_eq!(viewer.recv(Duration::from_secs(1)).unwrap(), Some(ServerToViewer::Joined));

        host.send(&HostToServer::Relay { data: "abcd".into() }).unwrap();
        assert_eq!(
            viewer.recv(Duration::from_secs(1)).unwrap(),
            Some(ServerToViewer::Relay { data: "abcd".into() })
        );

        viewer.send(&ViewerToServer::Relay { data: "ef01".into() }).unwrap();
        assert_eq!(host.recv(Duration::from_secs(1)).unwrap(), Some(ServerToHost::Relay { data: "ef01".into() }));

        assert_eq!(hub.host_relays(), vec![vec![0xab, 0xcd]]);
    }

    #[test]
    fn relay_with_no_viewer_is_dropped() {
        let (hub, mut host) = FakeHub::new();
        host.send(&HostToServer::Relay { data: "aa".into() }).unwrap();
        assert_eq!(hub.host_relays(), vec![vec![0xaa]]);
        // 버려졌으니 아무도 못 받지만, 기록은 남는다 — 여기서는 그냥 panic 하지 않는지만 본다.
    }

    #[test]
    fn kick_closes_viewer() {
        let (hub, mut host) = FakeHub::new();
        let mut viewer = hub.viewer();
        assert_eq!(host.recv(Duration::from_secs(1)).unwrap(), Some(ServerToHost::ViewerJoined));
        assert_eq!(viewer.recv(Duration::from_secs(1)).unwrap(), Some(ServerToViewer::Joined));

        host.send(&HostToServer::Relay { data: "aa".into() }).unwrap();
        host.send(&HostToServer::Kick).unwrap();

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
        let viewer = hub.viewer();
        assert_eq!(host.recv(Duration::from_secs(1)).unwrap(), Some(ServerToHost::ViewerJoined));
        drop(viewer);
        assert_eq!(host.recv(Duration::from_secs(1)).unwrap(), Some(ServerToHost::ViewerLeft));

        let (hub2, host2) = FakeHub::new();
        let mut viewer2 = hub2.viewer();
        assert_eq!(viewer2.recv(Duration::from_secs(1)).unwrap(), Some(ServerToViewer::Joined));
        drop(host2);
        assert_eq!(viewer2.recv(Duration::from_secs(1)).unwrap(), Some(ServerToViewer::HostLeft));
        assert!(matches!(
            viewer2.recv(Duration::from_secs(1)),
            Err(LinkError::Closed { code: Some(4002) })
        ));
    }

    #[test]
    fn new_viewer_replaces_old_one() {
        let (hub, mut host) = FakeHub::new();
        let mut v1 = hub.viewer();
        assert_eq!(host.recv(Duration::from_secs(1)).unwrap(), Some(ServerToHost::ViewerJoined));
        assert_eq!(v1.recv(Duration::from_secs(1)).unwrap(), Some(ServerToViewer::Joined));

        let mut v2 = hub.viewer();
        assert!(matches!(v1.recv(Duration::from_secs(1)), Err(LinkError::Closed { code: Some(4001) })));
        assert_eq!(host.recv(Duration::from_secs(1)).unwrap(), Some(ServerToHost::ViewerJoined));
        assert_eq!(v2.recv(Duration::from_secs(1)).unwrap(), Some(ServerToViewer::Joined));

        // 옛 viewer 를 드롭해도 이미 교체됐으므로 host 는 ViewerLeft 를 다시 받지 않는다.
        drop(v1);
        assert!(host.recv(Duration::from_millis(100)).unwrap().is_none());
    }
}
