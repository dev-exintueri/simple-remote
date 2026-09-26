use serde::{Deserialize, Serialize};

/// viewer 가 붙을 때 원하는 접속 종류. host 의 현재 [`HostMode`] 와 맞아야 받아들여진다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JoinKind {
    New,
    Reconnect,
}

/// host 가 지금 받을 접속 종류. `new` 를 보내기 전까지는 `reconnect` 로 취급한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostMode {
    New,
    Reconnect,
}

/// signaling 서버(Cloudflare Workers)와 host 사이의 메시지.
/// 새 등록은 host 가 서명 전에 자기 ID 를 모르므로, challenge 에 서버가 고른 ID 를 싣는다.
/// `n` 은 서버가 접속마다 매기는 1 부터 시작하는 번호로, 늦게 도착한 relay/kick 이
/// 다음 viewer 에게 잘못 적용되지 않게 막는다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ServerToHost {
    Challenge { nonce: String, id: String },
    Registered { id: String },
    ViewerJoined { n: u64, kind: JoinKind },
    ViewerLeft { n: u64 },
    Relay { n: u64, data: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum HostToServer {
    Auth { sig: String },
    Mode { mode: HostMode },
    Relay { n: u64, data: String },
    Kick { n: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ServerToViewer {
    Joined,
    Relay { data: String },
    HostLeft,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ViewerToServer {
    Relay { data: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signaling_json_shapes() {
        assert_eq!(
            serde_json::to_string(&HostToServer::Kick { n: 2 }).unwrap(),
            r#"{"t":"kick","n":2}"#
        );
        assert_eq!(
            serde_json::to_string(&HostToServer::Mode { mode: HostMode::New }).unwrap(),
            r#"{"t":"mode","mode":"new"}"#
        );
        assert_eq!(
            serde_json::to_string(&HostToServer::Relay { n: 1, data: "ab".into() }).unwrap(),
            r#"{"t":"relay","n":1,"data":"ab"}"#
        );
        assert_eq!(
            serde_json::to_string(&HostToServer::Auth { sig: "ab".into() }).unwrap(),
            r#"{"t":"auth","sig":"ab"}"#
        );
        assert_eq!(
            serde_json::to_string(&ViewerToServer::Relay { data: "00".into() }).unwrap(),
            r#"{"t":"relay","data":"00"}"#
        );
        let m: ServerToHost =
            serde_json::from_str(r#"{"t":"viewer_joined","n":3,"kind":"reconnect"}"#).unwrap();
        assert_eq!(m, ServerToHost::ViewerJoined { n: 3, kind: JoinKind::Reconnect });
        let m: ServerToHost = serde_json::from_str(r#"{"t":"viewer_left","n":3}"#).unwrap();
        assert_eq!(m, ServerToHost::ViewerLeft { n: 3 });
        let m: ServerToViewer = serde_json::from_str(r#"{"t":"host_left"}"#).unwrap();
        assert_eq!(m, ServerToViewer::HostLeft);
        let m: ServerToHost =
            serde_json::from_str(r#"{"t":"registered","id":"123456789"}"#).unwrap();
        assert_eq!(m, ServerToHost::Registered { id: "123456789".into() });
        let m: ServerToHost =
            serde_json::from_str(r#"{"t":"challenge","nonce":"00ff","id":"123456789"}"#).unwrap();
        assert_eq!(
            m,
            ServerToHost::Challenge {
                nonce: "00ff".into(),
                id: "123456789".into()
            }
        );
    }
}
