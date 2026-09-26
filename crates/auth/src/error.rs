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
}
