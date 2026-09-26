use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, KeyInit, Nonce};
use zeroize::Zeroize;

use crate::AuthError;
use crate::pake::SessionKeys;

const MIN_SEALED_LEN: usize = 8 + 16;

fn nonce_for(counter: u64) -> Nonce {
    let mut bytes = [0u8; 12];
    bytes[4..].copy_from_slice(&counter.to_be_bytes());
    Nonce::from(bytes)
}

/// 한 방향으로 메시지를 봉인해 보내는 쪽. key 는 drop 시 zeroize 된다.
pub struct SealedSender {
    cipher: ChaCha20Poly1305,
    label: &'static [u8],
    counter: u64,
}

impl SealedSender {
    fn new(mut key: [u8; 32], label: &'static [u8]) -> SealedSender {
        let cipher = ChaCha20Poly1305::new(&Key::from(key));
        key.zeroize();
        SealedSender {
            cipher,
            label,
            counter: 0,
        }
    }

    /// 출력 = `u64_be(counter) || ChaCha20Poly1305(ciphertext+tag)`.
    ///
    /// counter 는 절대 재사용하지 않는다: u64::MAX 에서 다음 호출은 오버플로로
    /// panic 한다 (2^64 개 메시지를 보내는 것은 현실적으로 불가능하므로 도달하지 않음).
    pub fn seal(&mut self, plaintext: &[u8]) -> Vec<u8> {
        let nonce = nonce_for(self.counter);
        let ciphertext = self
            .cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: plaintext,
                    aad: self.label,
                },
            )
            .expect("chacha20poly1305 encryption does not fail for valid inputs");
        let mut out = Vec::with_capacity(8 + ciphertext.len());
        out.extend_from_slice(&self.counter.to_be_bytes());
        out.extend_from_slice(&ciphertext);
        self.counter = self
            .counter
            .checked_add(1)
            .expect("counter must never wrap and reuse a nonce");
        out
    }
}

/// 한 방향으로 온 메시지를 여는 쪽. key 는 drop 시 zeroize 된다.
pub struct SealedReceiver {
    cipher: ChaCha20Poly1305,
    label: &'static [u8],
    expected_counter: u64,
}

impl SealedReceiver {
    fn new(mut key: [u8; 32], label: &'static [u8]) -> SealedReceiver {
        let cipher = ChaCha20Poly1305::new(&Key::from(key));
        key.zeroize();
        SealedReceiver {
            cipher,
            label,
            expected_counter: 0,
        }
    }

    /// counter 가 기대값과 정확히 같을 때만 열고 기대값을 1 올린다.
    /// 실패하면 `BadSeal` 이고 상태는 그대로.
    pub fn open(&mut self, msg: &[u8]) -> Result<Vec<u8>, AuthError> {
        if msg.len() < MIN_SEALED_LEN {
            return Err(AuthError::BadSeal);
        }
        let counter = u64::from_be_bytes(msg[..8].try_into().unwrap());
        if counter != self.expected_counter {
            return Err(AuthError::BadSeal);
        }
        let nonce = nonce_for(counter);
        let plaintext = self
            .cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: &msg[8..],
                    aad: self.label,
                },
            )
            .map_err(|_| AuthError::BadSeal)?;
        self.expected_counter = self
            .expected_counter
            .checked_add(1)
            .expect("counter must never wrap and reuse a nonce");
        Ok(plaintext)
    }
}

impl SessionKeys {
    /// viewer 쪽: 보냄 = viewer->host key (label `v2h`), 받음 = host->viewer key (label `h2v`).
    pub fn viewer_side(&self) -> (SealedSender, SealedReceiver) {
        (
            SealedSender::new(self.viewer_to_host, b"v2h"),
            SealedReceiver::new(self.host_to_viewer, b"h2v"),
        )
    }

    /// host 쪽: 보냄 = host->viewer key (label `h2v`), 받음 = viewer->host key (label `v2h`).
    pub fn host_side(&self) -> (SealedSender, SealedReceiver) {
        (
            SealedSender::new(self.host_to_viewer, b"h2v"),
            SealedReceiver::new(self.viewer_to_host, b"v2h"),
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::{AuthError, SealedReceiver, SealedSender, SessionKeys};

    fn pair() -> ((SealedSender, SealedReceiver), (SealedSender, SealedReceiver)) {
        let k = SessionKeys {
            viewer_to_host: [1; 32],
            host_to_viewer: [2; 32],
            hello_binding: [3; 32],
        };
        (k.viewer_side(), k.host_side())
    }

    #[test]
    fn roundtrip_both_directions() {
        let ((mut vs, mut vr), (mut hs, mut hr)) = pair();
        assert_eq!(hr.open(&vs.seal(b"offer")).unwrap(), b"offer");
        assert_eq!(vr.open(&hs.seal(b"answer")).unwrap(), b"answer");
    }

    #[test]
    fn replay_rejected() {
        let ((mut vs, _), (_, mut hr)) = pair();
        let m = vs.seal(b"a");
        hr.open(&m).unwrap();
        assert!(matches!(hr.open(&m), Err(AuthError::BadSeal)));
    }

    #[test]
    fn reorder_rejected() {
        let ((mut vs, _), (_, mut hr)) = pair();
        let (m0, m1) = (vs.seal(b"0"), vs.seal(b"1"));
        assert!(hr.open(&m1).is_err());
        assert_eq!(hr.open(&m0).unwrap(), b"0"); // 실패가 상태를 바꾸지 않음
    }

    #[test]
    fn tamper_rejected() {
        let ((mut vs, _), (_, mut hr)) = pair();
        let mut m = vs.seal(b"offer");
        let last = m.len() - 1;
        m[last] ^= 1;
        assert!(hr.open(&m).is_err());
        assert!(hr.open(&[0u8; 5]).is_err()); // 너무 짧음
    }

    #[test]
    fn wrong_direction_rejected() {
        let ((mut vs, _), (_, _)) = pair();
        let ((_, mut vr), _) = pair();
        assert!(vr.open(&vs.seal(b"x")).is_err()); // viewer 가 자기 방향 메시지를 받음
    }
}
