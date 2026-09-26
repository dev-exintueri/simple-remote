use serde::{Deserialize, Serialize};

/// signaling 서버(Cloudflare Workers)와 host 사이의 메시지.
/// 새 등록은 host 가 서명 전에 자기 ID 를 모르므로, challenge 에 서버가 고른 ID 를 싣는다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ServerToHost {
    Challenge { nonce: String, id: String },
    Registered { id: String },
    ViewerJoined,
    ViewerLeft,
    Relay { data: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum HostToServer {
    Auth { sig: String },
    Relay { data: String },
    Kick,
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
            serde_json::to_string(&HostToServer::Kick).unwrap(),
            r#"{"t":"kick"}"#
        );
        assert_eq!(
            serde_json::to_string(&HostToServer::Auth { sig: "ab".into() }).unwrap(),
            r#"{"t":"auth","sig":"ab"}"#
        );
        assert_eq!(
            serde_json::to_string(&ViewerToServer::Relay { data: "00".into() }).unwrap(),
            r#"{"t":"relay","data":"00"}"#
        );
        let m: ServerToHost = serde_json::from_str(r#"{"t":"viewer_joined"}"#).unwrap();
        assert_eq!(m, ServerToHost::ViewerJoined);
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
