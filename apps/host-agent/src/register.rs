//! signaling 서버에 host 를 등록한다: challenge 에 기기 key 로 서명하고 ID 를 받는다.

use std::time::Duration;

use auth::DeviceKeys;
use protocol::signaling::{HostToServer, ServerToHost};
use transport::{Link, LinkError, WsLink};

const AUTH_PREFIX: &[u8] = b"simple-remote signaling auth v1";
const STEP: Duration = Duration::from_secs(15);

#[derive(Debug, thiserror::Error)]
pub enum RegisterError {
    #[error("signaling error: {0}")]
    Link(#[from] LinkError),
    #[error("unexpected registration message: {0}")]
    Protocol(&'static str),
}

/// `id` 가 `None` 이면 새 ID 를 받고, `Some` 이면 그 ID 로 다시 등록한다.
/// 돌려주는 link 는 등록이 끝나 `ViewerJoined` 를 기다리는 상태다.
pub fn register_host(
    server: &str,
    keys: &DeviceKeys,
    id: Option<&str>,
) -> Result<(String, WsLink<ServerToHost, HostToServer>), RegisterError> {
    let key = hex::encode(keys.device_public());
    let url = match id {
        None => format!("{server}/v1/host?key={key}"),
        Some(id) => {
            if !is_host_id(id) {
                return Err(RegisterError::Protocol("bad host id"));
            }
            format!("{server}/v1/host/{id}?key={key}")
        }
    };
    let mut link = WsLink::connect(&url)?;
    let (nonce, challenge_id) = match link.recv(STEP)? {
        Some(ServerToHost::Challenge { nonce, id }) => (nonce, id),
        Some(_) => return Err(RegisterError::Protocol("expected challenge")),
        None => return Err(RegisterError::Protocol("no challenge")),
    };
    let nonce = hex::decode(&nonce)
        .ok()
        .filter(|n| n.len() == 32)
        .ok_or(RegisterError::Protocol("bad challenge nonce"))?;
    if !is_host_id(&challenge_id) {
        return Err(RegisterError::Protocol("bad challenge id"));
    }
    if id.is_some_and(|id| id != challenge_id) {
        return Err(RegisterError::Protocol("challenge for another id"));
    }

    let mut signed = Vec::with_capacity(AUTH_PREFIX.len() + nonce.len() + challenge_id.len());
    signed.extend_from_slice(AUTH_PREFIX);
    signed.extend_from_slice(&nonce);
    signed.extend_from_slice(challenge_id.as_bytes());
    link.send(&HostToServer::Auth { sig: hex::encode(keys.sign_raw(&signed)) })?;

    match link.recv(STEP)? {
        Some(ServerToHost::Registered { id }) if id == challenge_id => Ok((id, link)),
        Some(ServerToHost::Registered { .. }) => Err(RegisterError::Protocol("registered another id")),
        Some(_) => Err(RegisterError::Protocol("expected registered")),
        None => Err(RegisterError::Protocol("no registered")),
    }
}

/// 서버가 발급하는 접속 ID 모양: 숫자 9자리.
fn is_host_id(id: &str) -> bool {
    id.len() == 9 && id.bytes().all(|b| b.is_ascii_digit())
}
