#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("wrong code")]
    WrongCode,
    #[error("malformed: {0}")]
    Malformed(&'static str),
    #[error("bad signature")]
    BadSignature,
    #[error("bad seal")]
    BadSeal,
    #[error("name too long")]
    NameTooLong,
    /// 재접속 상대를 어느 허가증으로도 확인하지 못함.
    #[error("unknown peer")]
    UnknownPeer,
    /// Noise handshake 메시지 처리 실패.
    #[error("handshake failed")]
    Handshake,
}
