//! 저장된 허가증으로 하는 재접속 key 합의 (Noise KK).
//! viewer 가 initiator, host 가 responder. 첫 메시지 payload 는 허가증 번호,
//! 두 번째 메시지 payload 는 비어 있다.

use snow::params::NoiseParams;
use snow::{Builder, HandshakeState, TransportState};
use zeroize::Zeroize;

use crate::{AuthError, DeviceKeys};

const PARAMS: &str = "Noise_KK_25519_ChaChaPoly_BLAKE2s";
const PROLOGUE_PREFIX: &[u8] = b"simple-remote reconnect v1\0";
/// Noise 메시지 최대 길이.
const MAX_NOISE_MSG: usize = 65535;
const TAG_LEN: usize = 16;
/// host 가 확인해 보는 허가증 후보 수 상한 (설계 제안값).
const MAX_CANDIDATES: usize = 64;

/// handshake 가 끝난 Noise 암호 통로. 메시지마다 nonce 가 올라가므로
/// 순서가 어긋나거나 다시 보낸 메시지는 풀리지 않는다.
pub struct NoiseChannel {
    transport: TransportState,
    buf: Vec<u8>,
}

impl std::fmt::Debug for NoiseChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NoiseChannel").finish_non_exhaustive()
    }
}

impl NoiseChannel {
    fn new(transport: TransportState) -> NoiseChannel {
        NoiseChannel { transport, buf: vec![0u8; MAX_NOISE_MSG] }
    }

    /// # Panics
    /// `plaintext` 가 Noise 한도 (65535 - 16 byte) 를 넘으면 panic 한다.
    /// 호출자는 SDP (수 KB) 만 봉한다.
    pub fn seal(&mut self, plaintext: &[u8]) -> Vec<u8> {
        assert!(
            plaintext.len() <= MAX_NOISE_MSG - TAG_LEN,
            "Noise 메시지 한도를 넘는 평문"
        );
        let n = self
            .transport
            .write_message(plaintext, &mut self.buf)
            .expect("한도 안의 평문은 암호화된다");
        self.buf[..n].to_vec()
    }

    /// 변조, 재전송, 순서 어긋남은 모두 `BadSeal`.
    pub fn open(&mut self, msg: &[u8]) -> Result<Vec<u8>, AuthError> {
        let n = self
            .transport
            .read_message(msg, &mut self.buf)
            .map_err(|_| AuthError::BadSeal)?;
        let plain = self.buf[..n].to_vec();
        self.buf[..n].zeroize();
        Ok(plain)
    }
}

/// 재접속 handshake 결과. `binding` 은 Noise handshake hash 로, Hello 서명에 묶는다.
pub struct ReconnectSession {
    pub binding: [u8; 32],
    pub channel: NoiseChannel,
}

impl std::fmt::Debug for ReconnectSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReconnectSession").finish_non_exhaustive()
    }
}

/// viewer 쪽 재접속 handshake. 두 번째 메시지를 기다리는 상태.
pub struct ViewerReconnect {
    hs: HandshakeState,
}

impl std::fmt::Debug for ViewerReconnect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ViewerReconnect").finish_non_exhaustive()
    }
}

impl ViewerReconnect {
    /// Noise 첫 메시지 (payload = `permit_id`) 를 만든다.
    pub fn start(
        keys: &DeviceKeys,
        host_id: &str,
        host_reconnect_key: &[u8; 32],
        permit_id: &[u8; 16],
    ) -> Result<(ViewerReconnect, Vec<u8>), AuthError> {
        let mut hs = build(keys, host_id, host_reconnect_key, true)?;
        let mut buf = vec![0u8; MAX_NOISE_MSG];
        let n = hs
            .write_message(permit_id, &mut buf)
            .map_err(|_| AuthError::Handshake)?;
        buf.truncate(n);
        Ok((ViewerReconnect { hs }, buf))
    }

    /// host 의 두 번째 메시지를 확인한다. 실패하면 `Handshake`.
    pub fn finish(mut self, msg2: &[u8]) -> Result<ReconnectSession, AuthError> {
        let mut buf = vec![0u8; MAX_NOISE_MSG];
        let n = self
            .hs
            .read_message(msg2, &mut buf)
            .map_err(|_| AuthError::Handshake)?;
        if n != 0 {
            return Err(AuthError::Handshake);
        }
        into_session(self.hs)
    }
}

/// host 쪽 재접속 handshake.
pub struct HostReconnect;

impl HostReconnect {
    /// `candidates` 는 (허가증 번호, viewer 재접속 공개 key). 앞에서부터 최대 64개를
    /// 차례로 시도해, 첫 메시지가 그 key 로 풀리고 payload 가 그 번호와 같은 첫 후보를 고른다.
    /// 돌려주는 값은 (세션, 확인된 허가증 번호, 두 번째 메시지). 맞는 후보가 없으면 `UnknownPeer`.
    pub fn respond(
        keys: &DeviceKeys,
        host_id: &str,
        msg1: &[u8],
        candidates: &[([u8; 16], [u8; 32])],
    ) -> Result<(ReconnectSession, [u8; 16], Vec<u8>), AuthError> {
        if msg1.len() > MAX_NOISE_MSG {
            return Err(AuthError::UnknownPeer);
        }
        let mut payload = vec![0u8; MAX_NOISE_MSG];
        for (permit_id, viewer_key) in candidates.iter().take(MAX_CANDIDATES) {
            let mut hs = build(keys, host_id, viewer_key, false)?;
            let Ok(n) = hs.read_message(msg1, &mut payload) else {
                continue;
            };
            if payload[..n] != permit_id[..] {
                continue;
            }
            let mut msg2 = vec![0u8; MAX_NOISE_MSG];
            let m = hs
                .write_message(&[], &mut msg2)
                .map_err(|_| AuthError::Handshake)?;
            msg2.truncate(m);
            return Ok((into_session(hs)?, *permit_id, msg2));
        }
        Err(AuthError::UnknownPeer)
    }
}

fn build(
    keys: &DeviceKeys,
    host_id: &str,
    remote: &[u8; 32],
    initiator: bool,
) -> Result<HandshakeState, AuthError> {
    let params: NoiseParams = PARAMS.parse().map_err(|_| AuthError::Handshake)?;
    let mut prologue = Vec::with_capacity(PROLOGUE_PREFIX.len() + host_id.len());
    prologue.extend_from_slice(PROLOGUE_PREFIX);
    prologue.extend_from_slice(host_id.as_bytes());
    let secret = keys.reconnect_secret_bytes();
    let builder = Builder::new(params)
        .prologue(&prologue)
        .and_then(|b| b.local_private_key(&secret[..]))
        .and_then(|b| b.remote_public_key(remote))
        .map_err(|_| AuthError::Handshake)?;
    let hs = if initiator {
        builder.build_initiator()
    } else {
        builder.build_responder()
    };
    drop(secret);
    hs.map_err(|_| AuthError::Handshake)
}

fn into_session(hs: HandshakeState) -> Result<ReconnectSession, AuthError> {
    if !hs.is_handshake_finished() {
        return Err(AuthError::Handshake);
    }
    let binding: [u8; 32] = hs
        .get_handshake_hash()
        .try_into()
        .map_err(|_| AuthError::Handshake)?;
    let transport = hs.into_transport_mode().map_err(|_| AuthError::Handshake)?;
    Ok(ReconnectSession { binding, channel: NoiseChannel::new(transport) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnect_roundtrip_and_seal() {
        let (v, h) = (DeviceKeys::generate("v").unwrap(), DeviceKeys::generate("h").unwrap());
        let id = [7u8; 16];
        let (vr, m1) = ViewerReconnect::start(&v, "123456789", &h.reconnect_public(), &id).unwrap();
        let cands = [([1u8; 16], [9u8; 32]), (id, v.reconnect_public())];
        let (mut hs, got, m2) = HostReconnect::respond(&h, "123456789", &m1, &cands).unwrap();
        assert_eq!(got, id);
        let mut vs = vr.finish(&m2).unwrap();
        assert_eq!(vs.binding, hs.binding);
        let c = vs.channel.seal(b"offer");
        assert_eq!(hs.channel.open(&c).unwrap(), b"offer");
        assert!(hs.channel.open(&c).is_err()); // 재전송 거부
        let a = hs.channel.seal(b"answer");
        assert_eq!(vs.channel.open(&a).unwrap(), b"answer");
    }

    #[test]
    fn unknown_viewer_key_is_refused() {
        let (v, h) = (DeviceKeys::generate("v").unwrap(), DeviceKeys::generate("h").unwrap());
        let other = DeviceKeys::generate("o").unwrap();
        let id = [7u8; 16];
        let (_, m1) = ViewerReconnect::start(&v, "123456789", &h.reconnect_public(), &id).unwrap();
        let cands = [([1u8; 16], [9u8; 32]), (id, other.reconnect_public())];
        assert!(matches!(
            HostReconnect::respond(&h, "123456789", &m1, &cands),
            Err(AuthError::UnknownPeer)
        ));
    }

    #[test]
    fn payload_must_match_candidate_id() {
        let (v, h) = (DeviceKeys::generate("v").unwrap(), DeviceKeys::generate("h").unwrap());
        let (_, m1) = ViewerReconnect::start(&v, "123456789", &h.reconnect_public(), &[7u8; 16]).unwrap();
        let cands = [([8u8; 16], v.reconnect_public())];
        assert!(matches!(
            HostReconnect::respond(&h, "123456789", &m1, &cands),
            Err(AuthError::UnknownPeer)
        ));
    }

    #[test]
    fn viewer_with_wrong_host_key_is_refused() {
        let (v, h) = (DeviceKeys::generate("v").unwrap(), DeviceKeys::generate("h").unwrap());
        let other = DeviceKeys::generate("o").unwrap();
        let id = [7u8; 16];
        let (_, m1) = ViewerReconnect::start(&v, "123456789", &other.reconnect_public(), &id).unwrap();
        let cands = [(id, v.reconnect_public())];
        assert!(matches!(
            HostReconnect::respond(&h, "123456789", &m1, &cands),
            Err(AuthError::UnknownPeer)
        ));
    }

    #[test]
    fn host_id_is_bound() {
        let (v, h) = (DeviceKeys::generate("v").unwrap(), DeviceKeys::generate("h").unwrap());
        let id = [7u8; 16];
        let (_, m1) = ViewerReconnect::start(&v, "111111111", &h.reconnect_public(), &id).unwrap();
        let cands = [(id, v.reconnect_public())];
        assert!(matches!(
            HostReconnect::respond(&h, "222222222", &m1, &cands),
            Err(AuthError::UnknownPeer)
        ));
    }

    #[test]
    fn tampered_second_message_fails() {
        let (v, h) = (DeviceKeys::generate("v").unwrap(), DeviceKeys::generate("h").unwrap());
        let id = [7u8; 16];
        let (vr, m1) = ViewerReconnect::start(&v, "123456789", &h.reconnect_public(), &id).unwrap();
        let cands = [(id, v.reconnect_public())];
        let (_, _, mut m2) = HostReconnect::respond(&h, "123456789", &m1, &cands).unwrap();
        *m2.last_mut().unwrap() ^= 1;
        assert!(matches!(vr.finish(&m2), Err(AuthError::Handshake)));
    }

    #[test]
    fn only_first_64_candidates_are_tried_and_oversized_msg1_refused() {
        let (v, h) = (DeviceKeys::generate("v").unwrap(), DeviceKeys::generate("h").unwrap());
        let id = [7u8; 16];
        let (_, m1) = ViewerReconnect::start(&v, "123456789", &h.reconnect_public(), &id).unwrap();
        let mut cands = vec![([1u8; 16], [9u8; 32]); 64];
        cands.push((id, v.reconnect_public()));
        assert!(matches!(
            HostReconnect::respond(&h, "123456789", &m1, &cands),
            Err(AuthError::UnknownPeer)
        ));
        cands.swap(63, 64);
        assert!(HostReconnect::respond(&h, "123456789", &m1, &cands).is_ok());

        let big = vec![0u8; 65536];
        assert!(matches!(
            HostReconnect::respond(&h, "123456789", &big, &cands),
            Err(AuthError::UnknownPeer)
        ));
        assert!(matches!(
            HostReconnect::respond(&h, "123456789", &[], &cands),
            Err(AuthError::UnknownPeer)
        ));
    }
}
