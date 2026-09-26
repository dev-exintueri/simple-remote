use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::identity::PeerIdentity;

/// 허가증이 살아 있는 기간. `Waiting` 상태가 이 시간을 넘기면 만료된다.
pub const PERMIT_TTL: Duration = Duration::from_secs(3_600);

/// `SystemTime` 을 epoch 기준 밀리초로 바꾼다. epoch 이전 시각은 0 으로 다룬다.
pub fn unix_ms(t: SystemTime) -> u64 {
    match t.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_millis() as u64,
        Err(_) => 0,
    }
}

fn is_expired(since_ms: u64, now_ms: u64) -> bool {
    now_ms >= since_ms.saturating_add(PERMIT_TTL.as_millis() as u64)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermitState {
    Session,
    Waiting { since_ms: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostPermit {
    pub id: [u8; 16],
    pub viewer: PeerIdentity,
    pub state: PermitState,
}

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum PermitError {
    #[error("unknown permit")]
    Unknown,
    #[error("permit expired")]
    Expired,
    #[error("permit already in session")]
    InSession,
    #[error("wrong device")]
    WrongDevice,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HostBook {
    permits: Vec<HostPermit>,
    known: Vec<PeerIdentity>,
}

impl HostBook {
    /// 새 허가증을 발급하고 상태를 `Session` 으로 시작한다. 같은 기기 key 를
    /// 이미 알고 있으면 이름·재접속 key 를 갱신하고, 아니면 새로 기록한다.
    pub fn issue(&mut self, viewer: &PeerIdentity) -> [u8; 16] {
        let id = self.fresh_id();
        self.permits.push(HostPermit {
            id,
            viewer: viewer.clone(),
            state: PermitState::Session,
        });
        match self.known.iter_mut().find(|k| k.device_key == viewer.device_key) {
            Some(known) => {
                known.name = viewer.name.clone();
                known.reconnect_key = viewer.reconnect_key;
            }
            None => self.known.push(viewer.clone()),
        }
        id
    }

    fn fresh_id(&self) -> [u8; 16] {
        loop {
            let mut id = [0u8; 16];
            getrandom::fill(&mut id).expect("system RNG must succeed");
            if !self.permits.iter().any(|p| p.id == id) {
                return id;
            }
        }
    }

    pub fn is_known(&self, device_key: &[u8; 32]) -> bool {
        self.known.iter().any(|k| &k.device_key == device_key)
    }

    /// 만료 전 `Waiting` 허가증만, `since_ms` 최신 순으로 돌려준다.
    pub fn reconnect_candidates(&self, now: SystemTime) -> Vec<([u8; 16], [u8; 32])> {
        let now_ms = unix_ms(now);
        let mut candidates: Vec<(u64, [u8; 16], [u8; 32])> = self
            .permits
            .iter()
            .filter_map(|p| match p.state {
                PermitState::Waiting { since_ms } if !is_expired(since_ms, now_ms) => {
                    Some((since_ms, p.id, p.viewer.reconnect_key))
                }
                _ => None,
            })
            .collect();
        candidates.sort_by(|a, b| b.0.cmp(&a.0));
        candidates.into_iter().map(|(_, id, key)| (id, key)).collect()
    }

    pub fn begin_reconnect(
        &mut self,
        id: &[u8; 16],
        device_key: &[u8; 32],
        now: SystemTime,
    ) -> Result<(), PermitError> {
        // 검사 순서(Unknown, InSession, Expired, WrongDevice)는 밖에서 관찰할 수 없다:
        // 번호(id)는 Noise 재접속 payload 안에서만 오가므로, 실패 종류가 순서대로
        // 새는 정보는 그 payload 를 이미 열어 본 상대에게만 의미가 있다.
        let now_ms = unix_ms(now);
        let permit = self.permits.iter_mut().find(|p| &p.id == id).ok_or(PermitError::Unknown)?;
        let since_ms = match permit.state {
            PermitState::Session => return Err(PermitError::InSession),
            PermitState::Waiting { since_ms } => since_ms,
        };
        if is_expired(since_ms, now_ms) {
            return Err(PermitError::Expired);
        }
        if &permit.viewer.device_key != device_key {
            return Err(PermitError::WrongDevice);
        }
        permit.state = PermitState::Session;
        Ok(())
    }

    /// `Session` 인 허가증만 `Waiting` 으로 옮긴다. 이미 `Waiting` 이거나 없는 번호면
    /// 아무것도 하지 않는다 (이미 끊긴 뒤 다시 부르면 최초 끊긴 시각이 뒤로 밀리며
    /// 만료 시각이 늘어나 버리는 것을 막는다).
    pub fn end_session(&mut self, id: &[u8; 16], now: SystemTime) {
        if let Some(permit) = self.permits.iter_mut().find(|p| &p.id == id) {
            if permit.state == PermitState::Session {
                permit.state = PermitState::Waiting { since_ms: unix_ms(now) };
            }
        }
    }

    pub fn revoke(&mut self, id: &[u8; 16]) {
        self.permits.retain(|p| &p.id != id);
    }

    pub fn revoke_all(&mut self) {
        self.permits.clear();
    }

    /// 만료된 `Waiting` 허가증을 장부에서 삭제한다.
    pub fn prune(&mut self, now: SystemTime) {
        let now_ms = unix_ms(now);
        self.permits.retain(|p| match p.state {
            PermitState::Waiting { since_ms } => !is_expired(since_ms, now_ms),
            PermitState::Session => true,
        });
    }

    /// `Session` 이거나 만료 전 `Waiting` 인 허가증이 하나라도 있는가.
    pub fn has_valid(&self, now: SystemTime) -> bool {
        let now_ms = unix_ms(now);
        self.permits.iter().any(|p| match p.state {
            PermitState::Session => true,
            PermitState::Waiting { since_ms } => !is_expired(since_ms, now_ms),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewerPermit {
    pub host_id: String,
    pub permit_id: [u8; 16],
    pub host: PeerIdentity,
    pub disconnected_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ViewerBook {
    permits: Vec<ViewerPermit>,
}

impl ViewerBook {
    /// 같은 `host_id` 가 있으면 교체한다.
    pub fn store(&mut self, p: ViewerPermit) {
        match self.permits.iter_mut().find(|existing| existing.host_id == p.host_id) {
            Some(existing) => *existing = p,
            None => self.permits.push(p),
        }
    }

    pub fn get(&self, host_id: &str) -> Option<&ViewerPermit> {
        self.permits.iter().find(|p| p.host_id == host_id)
    }

    pub fn mark_disconnected(&mut self, host_id: &str, now: SystemTime) {
        if let Some(p) = self.permits.iter_mut().find(|p| p.host_id == host_id) {
            p.disconnected_ms = Some(unix_ms(now));
        }
    }

    pub fn remove(&mut self, host_id: &str) {
        self.permits.retain(|p| p.host_id != host_id);
    }

    /// 없는 host, 또는 끊긴 지 1시간이 지난 host 는 만료로 본다.
    /// `disconnected_ms` 가 `None` (세션 중이거나 끊긴 적 없음) 이면 만료가 아니다.
    pub fn expired(&self, host_id: &str, now: SystemTime) -> bool {
        match self.get(host_id) {
            None => true,
            Some(p) => match p.disconnected_ms {
                None => false,
                Some(disconnected_ms) => is_expired(disconnected_ms, unix_ms(now)),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_time() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_000_000)
    }

    fn viewer(name: &str) -> PeerIdentity {
        PeerIdentity {
            device_key: {
                let mut k = [0u8; 32];
                getrandom::fill(&mut k).unwrap();
                k
            },
            name: name.to_string(),
            reconnect_key: {
                let mut k = [0u8; 32];
                getrandom::fill(&mut k).unwrap();
                k
            },
        }
    }

    #[test]
    fn issue_then_end_then_reconnect() {
        let t = base_time();
        let mut host = HostBook::default();
        let v = viewer("뷰어");
        let id = host.issue(&v);

        assert!(host.reconnect_candidates(t).is_empty());

        host.end_session(&id, t);
        let candidates = host.reconnect_candidates(t);
        assert_eq!(candidates, vec![(id, v.reconnect_key)]);

        assert!(host.begin_reconnect(&id, &v.device_key, t + Duration::from_secs(59 * 60)).is_ok());
        assert!(host.reconnect_candidates(t + Duration::from_secs(59 * 60)).is_empty());
    }

    #[test]
    fn expiry_boundary_is_exclusive() {
        let t = base_time();
        let mut host = HostBook::default();
        let v = viewer("뷰어");
        let id = host.issue(&v);
        host.end_session(&id, t);

        let mut just_before = host.clone();
        assert!(
            just_before
                .begin_reconnect(&id, &v.device_key, t + PERMIT_TTL - Duration::from_millis(1))
                .is_ok()
        );

        assert_eq!(
            host.begin_reconnect(&id, &v.device_key, t + PERMIT_TTL),
            Err(PermitError::Expired)
        );
        assert!(host.reconnect_candidates(t + PERMIT_TTL).is_empty());
    }

    #[test]
    fn reconnect_restarts_the_hour() {
        let t = base_time();
        let mut host = HostBook::default();
        let v = viewer("뷰어");
        let id = host.issue(&v);

        host.end_session(&id, t);
        assert!(host.begin_reconnect(&id, &v.device_key, t + Duration::from_secs(50 * 60)).is_ok());

        host.end_session(&id, t + Duration::from_secs(100 * 60));
        assert!(
            host.begin_reconnect(
                &id,
                &v.device_key,
                t + Duration::from_secs(100 * 60) + Duration::from_secs(59 * 60)
            )
            .is_ok()
        );
    }

    #[test]
    fn end_session_twice_keeps_first_disconnect_time() {
        let t = base_time();
        let mut host = HostBook::default();
        let v = viewer("뷰어");
        let id = host.issue(&v);

        host.end_session(&id, t);
        host.end_session(&id, t + Duration::from_secs(50 * 60));

        assert_eq!(
            host.begin_reconnect(&id, &v.device_key, t + PERMIT_TTL),
            Err(PermitError::Expired)
        );
        assert!(host.reconnect_candidates(t + PERMIT_TTL).is_empty());
    }

    #[test]
    fn wrong_device_and_unknown() {
        let t = base_time();

        let mut host = HostBook::default();
        let v = viewer("뷰어");
        let id = host.issue(&v);
        host.end_session(&id, t);
        let other = viewer("다른 기기");
        assert_eq!(
            host.begin_reconnect(&id, &other.device_key, t + Duration::from_secs(60)),
            Err(PermitError::WrongDevice)
        );

        let unknown_id = [0xffu8; 16];
        assert_eq!(host.begin_reconnect(&unknown_id, &v.device_key, t), Err(PermitError::Unknown));

        let mut host2 = HostBook::default();
        let v2 = viewer("뷰어2");
        let id2 = host2.issue(&v2);
        assert_eq!(host2.begin_reconnect(&id2, &v2.device_key, t), Err(PermitError::InSession));
    }

    #[test]
    fn revoke_and_revoke_all_keep_known() {
        let t = base_time();
        let mut host = HostBook::default();
        let v = viewer("뷰어");
        let id = host.issue(&v);
        host.revoke(&id);
        assert_eq!(host.begin_reconnect(&id, &v.device_key, t), Err(PermitError::Unknown));
        assert!(host.is_known(&v.device_key));

        let v2 = viewer("뷰어2");
        let id2 = host.issue(&v2);
        let v3 = viewer("뷰어3");
        let id3 = host.issue(&v3);
        host.revoke_all();
        assert_eq!(host.begin_reconnect(&id2, &v2.device_key, t), Err(PermitError::Unknown));
        assert_eq!(host.begin_reconnect(&id3, &v3.device_key, t), Err(PermitError::Unknown));
        assert!(host.is_known(&v2.device_key));
        assert!(host.is_known(&v3.device_key));
    }

    #[test]
    fn has_valid_and_prune() {
        let t = base_time();
        let mut host = HostBook::default();
        assert!(!host.has_valid(t));

        let v = viewer("뷰어");
        let id = host.issue(&v);
        assert!(host.has_valid(t));

        host.end_session(&id, t);
        assert!(host.has_valid(t + Duration::from_secs(30 * 60)));
        assert!(!host.has_valid(t + PERMIT_TTL));

        host.prune(t + PERMIT_TTL);
        assert!(!host.has_valid(t + PERMIT_TTL));
        assert_eq!(host.begin_reconnect(&id, &v.device_key, t + PERMIT_TTL), Err(PermitError::Unknown));
    }

    #[test]
    fn viewer_book_store_replace_and_expired() {
        let t = base_time();
        let mut book = ViewerBook::default();
        let host_identity = viewer("호스트");
        let p = ViewerPermit {
            host_id: "h1".to_string(),
            permit_id: [1u8; 16],
            host: host_identity.clone(),
            disconnected_ms: None,
        };
        book.store(p);

        assert_eq!(book.get("h1").unwrap().disconnected_ms, None);
        assert!(!book.expired("h1", t));
        assert!(book.expired("nonexistent", t));

        book.mark_disconnected("h1", t);
        assert!(!book.expired("h1", t + Duration::from_secs(59 * 60)));
        assert!(book.expired("h1", t + PERMIT_TTL));

        let replacement = ViewerPermit {
            host_id: "h1".to_string(),
            permit_id: [2u8; 16],
            host: host_identity,
            disconnected_ms: None,
        };
        book.store(replacement);
        assert_eq!(book.get("h1").unwrap().permit_id, [2u8; 16]);
        assert!(!book.expired("h1", t + PERMIT_TTL));

        book.remove("h1");
        assert!(book.get("h1").is_none());
        assert!(book.expired("h1", t));
    }

    #[test]
    fn books_roundtrip_serde() {
        let t = base_time();
        let mut host = HostBook::default();
        let v = viewer("뷰어");
        let id = host.issue(&v);
        host.end_session(&id, t);
        let bytes = postcard::to_stdvec(&host).unwrap();
        assert_eq!(postcard::from_bytes::<HostBook>(&bytes).unwrap(), host);

        let mut viewer_book = ViewerBook::default();
        viewer_book.store(ViewerPermit {
            host_id: "h1".to_string(),
            permit_id: [1u8; 16],
            host: v,
            disconnected_ms: Some(unix_ms(t)),
        });
        let bytes = postcard::to_stdvec(&viewer_book).unwrap();
        assert_eq!(postcard::from_bytes::<ViewerBook>(&bytes).unwrap(), viewer_book);
    }
}
