use std::marker::PhantomData;
use std::net::TcpStream;
use std::sync::Once;
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde::Serialize;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

/// signaling 채널 하나(host 쪽 또는 viewer 쪽)를 추상화한다.
/// `In` 은 상대에서 오는 메시지 타입, `Out` 은 내가 보내는 메시지 타입이다.
pub trait Link<In, Out> {
    fn send(&mut self, msg: &Out) -> Result<(), LinkError>;
    /// `timeout` 안에 메시지가 없으면 `Ok(None)`.
    fn recv(&mut self, timeout: Duration) -> Result<Option<In>, LinkError>;
}

#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error("link closed (code {code:?})")]
    Closed { code: Option<u16> },
    #[error("http error before upgrade: {status}")]
    Http { status: u16 },
    #[error("insecure url (ws:// is only allowed on loopback)")]
    InsecureUrl,
    #[error("io error: {0}")]
    Io(std::io::Error),
    #[error("protocol error: {0}")]
    Protocol(String),
}

/// WebSocket 위의 [`Link`]. `wss://` 는 항상 허용하고, `ws://` 는 authority 의 host 가
/// `127.0.0.1`, `::1`, `localhost` 일 때만 허용한다 (그 밖은 [`LinkError::InsecureUrl`]).
pub struct WsLink<In, Out> {
    ws: WebSocket<MaybeTlsStream<TcpStream>>,
    closed: bool,
    _in: PhantomData<In>,
    _out: PhantomData<Out>,
}

impl<In, Out> WsLink<In, Out> {
    pub fn connect(url: &str) -> Result<Self, LinkError> {
        ensure_secure_url(url)?;
        install_rustls_provider();

        let (ws, _response) = tungstenite::connect(url).map_err(map_tungstenite_err)?;
        Ok(WsLink { ws, closed: false, _in: PhantomData, _out: PhantomData })
    }

    fn set_read_timeout(&self, dur: Option<Duration>) -> Result<(), LinkError> {
        match self.ws.get_ref() {
            MaybeTlsStream::Plain(s) => s.set_read_timeout(dur).map_err(LinkError::Io),
            MaybeTlsStream::Rustls(s) => s.get_ref().set_read_timeout(dur).map_err(LinkError::Io),
            _ => Err(LinkError::Protocol("unsupported stream type".into())),
        }
    }
}

impl<In: DeserializeOwned, Out: Serialize> Link<In, Out> for WsLink<In, Out> {
    fn send(&mut self, msg: &Out) -> Result<(), LinkError> {
        if self.closed {
            return Err(LinkError::Closed { code: None });
        }
        let text = serde_json::to_string(msg).map_err(|e| LinkError::Protocol(e.to_string()))?;
        match self.ws.send(Message::text(text)) {
            Ok(()) => Ok(()),
            Err(e) => Err(self.map_after_error(e)),
        }
    }

    fn recv(&mut self, timeout: Duration) -> Result<Option<In>, LinkError> {
        if self.closed {
            return Err(LinkError::Closed { code: None });
        }

        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(None);
            }
            self.set_read_timeout(Some(remaining))?;

            match self.ws.read() {
                Ok(Message::Text(text)) => {
                    let parsed = serde_json::from_str::<In>(&text)
                        .map_err(|e| LinkError::Protocol(e.to_string()))?;
                    return Ok(Some(parsed));
                }
                Ok(Message::Binary(_)) => {
                    return Err(LinkError::Protocol("unexpected binary frame".into()));
                }
                Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {
                    // tungstenite 가 다음 read/write 에 자동으로 pong 을 답하므로,
                    // 남은 시간이 있으면 계속 읽는다.
                    continue;
                }
                Ok(Message::Close(frame)) => {
                    self.closed = true;
                    return Err(LinkError::Closed { code: frame.map(|f| u16::from(f.code)) });
                }
                Ok(Message::Frame(_)) => {
                    return Err(LinkError::Protocol("raw frame from read()".into()));
                }
                Err(e) => {
                    if let tungstenite::Error::Io(io) = &e {
                        if is_timeout(io) {
                            return Ok(None);
                        }
                    }
                    return Err(self.map_after_error(e));
                }
            }
        }
    }
}

impl<In, Out> WsLink<In, Out> {
    /// send/recv 중 발생한 오류를 [`LinkError`] 로 옮기면서, 연결이 끝났으면 `closed` 를 표시한다.
    fn map_after_error(&mut self, e: tungstenite::Error) -> LinkError {
        match &e {
            tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed => {
                self.closed = true;
                LinkError::Closed { code: None }
            }
            tungstenite::Error::Io(io) => LinkError::Io(std::io::Error::new(io.kind(), io.to_string())),
            _ => LinkError::Protocol(e.to_string()),
        }
    }
}

fn is_timeout(io: &std::io::Error) -> bool {
    matches!(io.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut)
}

fn map_tungstenite_err(e: tungstenite::Error) -> LinkError {
    match e {
        tungstenite::Error::Http(response) => LinkError::Http { status: response.status().as_u16() },
        tungstenite::Error::Io(io) => LinkError::Io(io),
        other => LinkError::Protocol(other.to_string()),
    }
}

/// `rustls::crypto::ring::default_provider().install_default()` 를 process 당 한 번만 호출한다.
/// 없으면 첫 `wss://` 연결이 panic 한다. 이미 설치돼 있으면 오는 `Err` 는 무시한다.
fn install_rustls_provider() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

/// `wss://` 는 항상 허용. `ws://` 는 host 가 `127.0.0.1`, `::1`, `localhost` 일 때만 허용.
fn ensure_secure_url(url: &str) -> Result<(), LinkError> {
    if url.starts_with("wss://") {
        return Ok(());
    }
    if let Some(rest) = url.strip_prefix("ws://") {
        let authority = rest.split('/').next().unwrap_or("");
        let host = host_of(authority);
        if host == "127.0.0.1" || host == "::1" || host == "localhost" {
            return Ok(());
        }
    }
    Err(LinkError::InsecureUrl)
}

/// authority(`host:port` 또는 `[v6]:port`) 에서 host 부분만 뽑아낸다.
fn host_of(authority: &str) -> &str {
    if let Some(rest) = authority.strip_prefix('[') {
        if let Some(end) = rest.find(']') {
            return &rest[..end];
        }
    }
    authority.split(':').next().unwrap_or(authority)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_localhost_forms_are_secure() {
        assert!(ensure_secure_url("ws://localhost:8787/x").is_ok());
        assert!(ensure_secure_url("ws://[::1]:8787/x").is_ok());
        assert!(ensure_secure_url("ws://127.0.0.1/x").is_ok());
        assert!(ensure_secure_url("ws://127.0.0.1:8787/x").is_ok());
    }

    #[test]
    fn non_loopback_ws_and_other_schemes_are_insecure() {
        assert!(matches!(ensure_secure_url("ws://example.com/x"), Err(LinkError::InsecureUrl)));
        assert!(matches!(ensure_secure_url("http://127.0.0.1/x"), Err(LinkError::InsecureUrl)));
    }

    #[test]
    fn wss_is_always_secure() {
        assert!(ensure_secure_url("wss://example.com/x").is_ok());
    }
}
