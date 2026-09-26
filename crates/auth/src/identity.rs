use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use protocol::PROTOCOL_VERSION;
use protocol::control::Hello;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::AuthError;
use crate::rng::rng;

const RECONNECT_KEY_CONTEXT: &[u8] = b"simple-remote reconnect key v1";
const HELLO_CONTEXT: &[u8] = b"simple-remote hello v1";
const NAME_MAX_BYTES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Viewer,
    Host,
}

impl Role {
    pub fn byte(self) -> u8 {
        match self {
            Role::Viewer => 1,
            Role::Host => 2,
        }
    }
}

/// 이 기기의 장기 key 쌍. 개인 key 는 drop 시 zeroize 된다
/// (`ed25519-dalek` 의 `zeroize` feature, `x25519-dalek` `StaticSecret`).
pub struct DeviceKeys {
    signing_key: SigningKey,
    reconnect_secret: StaticSecret,
    name: String,
}

impl std::fmt::Debug for DeviceKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceKeys")
            .field("name", &self.name)
            .field("device_public", &self.device_public())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerIdentity {
    pub device_key: [u8; 32],
    pub name: String,
    pub reconnect_key: [u8; 32],
}

impl DeviceKeys {
    /// `name` 은 1~64 byte UTF-8 이어야 한다 (설계 제안값).
    pub fn generate(name: &str) -> Result<DeviceKeys, AuthError> {
        if name.is_empty() || name.len() > NAME_MAX_BYTES {
            return Err(AuthError::NameTooLong);
        }
        Ok(DeviceKeys {
            signing_key: SigningKey::generate(&mut rng()),
            reconnect_secret: StaticSecret::random_from_rng(&mut rng()),
            name: name.to_string(),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn device_public(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }

    pub fn reconnect_public(&self) -> [u8; 32] {
        PublicKey::from(&self.reconnect_secret).to_bytes()
    }

    pub fn sign_raw(&self, msg: &[u8]) -> [u8; 64] {
        self.signing_key.sign(msg).to_bytes()
    }

    pub fn make_hello(&self, role: Role, binding: &[u8; 32], own_fp: &[u8], peer_fp: &[u8]) -> Hello {
        let reconnect_public = self.reconnect_public();
        let mut reconnect_msg = Vec::with_capacity(RECONNECT_KEY_CONTEXT.len() + 32);
        reconnect_msg.extend_from_slice(RECONNECT_KEY_CONTEXT);
        reconnect_msg.extend_from_slice(&reconnect_public);
        let reconnect_key_sig = self.sign_raw(&reconnect_msg);

        let session_msg = session_message(role, binding, own_fp, peer_fp, self.name.as_bytes());
        let session_sig = self.sign_raw(&session_msg);

        Hello {
            protocol_version: PROTOCOL_VERSION,
            device_key: self.device_public().to_vec(),
            device_name: self.name.clone(),
            reconnect_key: reconnect_public.to_vec(),
            reconnect_key_sig: reconnect_key_sig.to_vec(),
            session_sig: session_sig.to_vec(),
        }
    }
}

fn session_message(role: Role, binding: &[u8; 32], own_fp: &[u8], peer_fp: &[u8], name: &[u8]) -> Vec<u8> {
    let mut msg = Vec::with_capacity(
        HELLO_CONTEXT.len() + 1 + 32 + 2 + own_fp.len() + 2 + peer_fp.len() + 2 + name.len(),
    );
    msg.extend_from_slice(HELLO_CONTEXT);
    msg.push(role.byte());
    msg.extend_from_slice(binding);
    msg.extend_from_slice(&(own_fp.len() as u16).to_be_bytes());
    msg.extend_from_slice(own_fp);
    msg.extend_from_slice(&(peer_fp.len() as u16).to_be_bytes());
    msg.extend_from_slice(peer_fp);
    msg.extend_from_slice(&(name.len() as u16).to_be_bytes());
    msg.extend_from_slice(name);
    msg
}

/// `hello` 를 검증하고, 확인된 상대 신원을 돌려준다.
///
/// `protocol_version` 은 확인하지 않는다 (연결 흐름 담당).
pub fn verify_hello(
    hello: &Hello,
    sender: Role,
    binding: &[u8; 32],
    sender_fp: &[u8],
    receiver_fp: &[u8],
) -> Result<PeerIdentity, AuthError> {
    let device_key: [u8; 32] = hello
        .device_key
        .as_slice()
        .try_into()
        .map_err(|_| AuthError::Malformed("device_key"))?;
    let reconnect_key: [u8; 32] = hello
        .reconnect_key
        .as_slice()
        .try_into()
        .map_err(|_| AuthError::Malformed("reconnect_key"))?;
    let reconnect_key_sig: [u8; 64] = hello
        .reconnect_key_sig
        .as_slice()
        .try_into()
        .map_err(|_| AuthError::Malformed("reconnect_key_sig"))?;
    let session_sig: [u8; 64] = hello
        .session_sig
        .as_slice()
        .try_into()
        .map_err(|_| AuthError::Malformed("session_sig"))?;

    let verifying_key =
        VerifyingKey::from_bytes(&device_key).map_err(|_| AuthError::Malformed("device_key"))?;

    let mut reconnect_msg = Vec::with_capacity(RECONNECT_KEY_CONTEXT.len() + 32);
    reconnect_msg.extend_from_slice(RECONNECT_KEY_CONTEXT);
    reconnect_msg.extend_from_slice(&reconnect_key);
    verifying_key
        .verify_strict(&reconnect_msg, &Signature::from_bytes(&reconnect_key_sig))
        .map_err(|_| AuthError::BadSignature)?;

    let session_msg = session_message(
        sender,
        binding,
        sender_fp,
        receiver_fp,
        hello.device_name.as_bytes(),
    );
    verifying_key
        .verify_strict(&session_msg, &Signature::from_bytes(&session_sig))
        .map_err(|_| AuthError::BadSignature)?;

    Ok(PeerIdentity {
        device_key,
        name: hello.device_name.clone(),
        reconnect_key,
    })
}

/// key 지문 표시용 문자열. SHA-256 앞 8 byte 를 4자리 hex 묶음 4개로.
pub fn key_fingerprint(key: &[u8; 32]) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(key);
    let hex = hex::encode(&hash[..8]);
    let chunks: Vec<&str> = hex
        .as_bytes()
        .chunks(4)
        .map(|c| std::str::from_utf8(c).unwrap())
        .collect();
    chunks.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (DeviceKeys, [u8; 32], Vec<u8>, Vec<u8>) {
        (
            DeviceKeys::generate("내 노트북").unwrap(),
            [9u8; 32],
            vec![1; 32],
            vec![2; 32],
        )
    }
    #[test]
    fn hello_roundtrip() {
        let (k, b, vfp, hfp) = setup();
        let h = k.make_hello(Role::Viewer, &b, &vfp, &hfp);
        let id = verify_hello(&h, Role::Viewer, &b, &vfp, &hfp).unwrap();
        assert_eq!(id.device_key, k.device_public());
        assert_eq!(id.reconnect_key, k.reconnect_public());
        assert_eq!(id.name, "내 노트북");
    }
    #[test]
    fn hello_rejects_tampering() {
        let (k, b, vfp, hfp) = setup();
        let h = k.make_hello(Role::Viewer, &b, &vfp, &hfp);
        assert!(verify_hello(&h, Role::Host, &b, &vfp, &hfp).is_err()); // 역할 바꿔치기
        assert!(verify_hello(&h, Role::Viewer, &[8u8; 32], &vfp, &hfp).is_err()); // 다른 세션
        assert!(verify_hello(&h, Role::Viewer, &b, &hfp, &vfp).is_err()); // 지문 뒤바꿈
        let mut n = h.clone();
        n.device_name = "다른 이름".into();
        assert!(verify_hello(&n, Role::Viewer, &b, &vfp, &hfp).is_err());
        let other = DeviceKeys::generate("x").unwrap();
        let mut r = h.clone();
        r.reconnect_key = other.reconnect_public().to_vec();
        assert!(verify_hello(&r, Role::Viewer, &b, &vfp, &hfp).is_err()); // 남의 재접속 key
        let mut s = h.clone();
        s.session_sig.truncate(63);
        assert!(matches!(
            verify_hello(&s, Role::Viewer, &b, &vfp, &hfp),
            Err(AuthError::Malformed(_))
        ));
    }
    #[test]
    fn name_limits_and_fingerprint_format() {
        assert!(matches!(
            DeviceKeys::generate(""),
            Err(AuthError::NameTooLong)
        ));
        assert!(DeviceKeys::generate(&"가".repeat(22)).is_err()); // 66 byte
        let fp = key_fingerprint(&[0u8; 32]);
        assert_eq!(fp.len(), 19);
        assert_eq!(fp.split(' ').count(), 4);
    }
}
