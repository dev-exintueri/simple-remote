use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("rtc error: {0}")]
    Rtc(String),
    #[error("channel write buffer full")]
    Backpressure,
    #[error("control channel not open")]
    NoChannel,
}
