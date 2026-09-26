use serde::{Deserialize, Serialize};

use crate::DecodeError;

/// control 채널로 주고받는 최대 메시지 크기 (설계 제안값).
pub const MAX_CONTROL_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Control {
    Hello(Hello),
    Decision(Decision),
    Ping { seq: u64 },
    Pong { seq: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    pub protocol_version: u32,
    pub device_key: Vec<u8>,
    pub device_name: String,
    // serde 가 `[u8; 64]` 를 derive 하지 못해 Vec<u8> 로 둔다. 길이는 auth 가 확인한다.
    pub reconnect_key: Vec<u8>,
    pub reconnect_key_sig: Vec<u8>,
    pub session_sig: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Decision {
    Accepted,
    Rejected(RejectReason),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RejectReason {
    Denied,
    NoResponse,
    VersionMismatch { host_version: u32 },
    BadIdentity,
}

pub fn encode(msg: &Control) -> Vec<u8> {
    postcard::to_allocvec(msg).expect("Control encode never fails")
}

pub fn decode(bytes: &[u8]) -> Result<Control, DecodeError> {
    if bytes.len() > MAX_CONTROL_BYTES {
        return Err(DecodeError::TooLarge {
            len: bytes.len(),
            max: MAX_CONTROL_BYTES,
        });
    }
    Ok(postcard::from_bytes(bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_roundtrip() {
        let msg = Control::Hello(Hello {
            protocol_version: 1,
            device_key: vec![1; 32],
            device_name: "노트북".into(),
            reconnect_key: vec![2; 32],
            reconnect_key_sig: vec![3; 64],
            session_sig: vec![4; 64],
        });
        assert_eq!(decode(&encode(&msg)).unwrap(), msg);
        let d = Control::Decision(Decision::Rejected(RejectReason::VersionMismatch {
            host_version: 7,
        }));
        assert_eq!(decode(&encode(&d)).unwrap(), d);
    }

    #[test]
    fn decode_rejects_garbage_and_oversize() {
        assert!(decode(&[0xff, 0xff, 0xff]).is_err());
        assert!(decode(&vec![0u8; MAX_CONTROL_BYTES + 1]).is_err());
    }
}
