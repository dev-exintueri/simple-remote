use std::time::Duration;

pub mod control;
pub mod pake_msg;
pub mod signaling;

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timeouts {
    pub signaling_step: Duration,
    pub connect: Duration,
    pub approval: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            signaling_step: Duration::from_secs(15),
            connect: Duration::from_secs(10),
            approval: Duration::from_secs(30),
        }
    }
}

/// 시간 초과가 난 단계. viewer 와 host 가 함께 씀.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Join,
    Pake,
    Answer,
    Connect,
    Hello,
    Decision,
    Ping,
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("message too large: {len} bytes (max {max})")]
    TooLarge { len: usize, max: usize },
    #[error("postcard decode failed: {0}")]
    Postcard(#[from] postcard::Error),
}
