use serde::{Deserialize, Serialize};

use crate::DecodeError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PakeMsg {
    Start { pa: Vec<u8> },
    Reply { pb: Vec<u8>, mac_b: Vec<u8> },
    Confirm { mac_a: Vec<u8>, sealed_offer: Vec<u8> },
    Answer { sealed_answer: Vec<u8> },
    RetryAfter { ms: u64 },
    Rejected,
}

pub fn encode(msg: &PakeMsg) -> Vec<u8> {
    postcard::to_allocvec(msg).expect("PakeMsg encode never fails")
}

pub fn decode(bytes: &[u8]) -> Result<PakeMsg, DecodeError> {
    Ok(postcard::from_bytes(bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pake_msg_roundtrip() {
        for m in [
            PakeMsg::Start { pa: vec![4; 65] },
            PakeMsg::RetryAfter { ms: 2000 },
            PakeMsg::Rejected,
        ] {
            assert_eq!(decode(&encode(&m)).unwrap(), m);
        }
    }
}
