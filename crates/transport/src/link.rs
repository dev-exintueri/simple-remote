use std::marker::PhantomData;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Once;
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde::Serialize;
use tungstenite::handshake::HandshakeError;
use tungstenite::protocol::WebSocketConfig;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

/// TCP 연결(주소마다)과 TLS + HTTP upgrade 의 read/write 한 번에 기다리는 상한 (설계 제안값).
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// 받는 WebSocket message/frame 상한. Worker 가 64 KiB 넘는 메시지를 막으므로 넉넉히 두 배.
const MAX_MESSAGE_BYTES: usize = 128 * 1024;

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
    /// `None` 이면 아직 안 닫힘. `Some(code)` 면 이미 닫혔고, 이후 모든 호출이 같은
    /// `code` 로 `Closed` 를 돌려준다.
    closed: Option<Option<u16>>,
    _in: PhantomData<In>,
    _out: PhantomData<Out>,
}

impl<In, Out> WsLink<In, Out> {
    /// 모든 대기에 상한이 있다: 주소마다 TCP 연결 [`CONNECT_TIMEOUT`], TLS + HTTP upgrade 의
    /// 각 read/write 도 [`CONNECT_TIMEOUT`]. write timeout 은 연결 뒤에도 남아 `send` 를 묶고,
    /// read timeout 은 `recv` 가 매번 다시 건다. [`MAX_MESSAGE_BYTES`] 를 넘는 메시지나 frame 은
    /// `recv` 에서 [`LinkError::Protocol`] 이 된다.
    pub fn connect(url: &str) -> Result<Self, LinkError> {
        let uri = ensure_secure_url(url)?;
        install_rustls_provider();

        let host = strip_ipv6_brackets(uri.host().ok_or(LinkError::InsecureUrl)?);
        let default_port = if uri.scheme_str().unwrap_or("").eq_ignore_ascii_case("wss") { 443 } else { 80 };
        let port = uri.port_u16().unwrap_or(default_port);
        let stream = connect_tcp(host, port)?;
        stream.set_nodelay(true).map_err(LinkError::Io)?;
        stream.set_read_timeout(Some(CONNECT_TIMEOUT)).map_err(LinkError::Io)?;
        stream.set_write_timeout(Some(CONNECT_TIMEOUT)).map_err(LinkError::Io)?;

        let config = WebSocketConfig::default()
            .max_message_size(Some(MAX_MESSAGE_BYTES))
            .max_frame_size(Some(MAX_MESSAGE_BYTES));
        let (ws, _response) = match tungstenite::client_tls_with_config(url, stream, Some(config), None) {
            Ok(pair) => pair,
            Err(HandshakeError::Failure(e)) => return Err(map_tungstenite_err(e)),
            // A blocking stream only "would block" when its read/write timeout fired (Unix
            // reports the timeout as WouldBlock; Windows as TimedOut, which comes as Failure).
            Err(HandshakeError::Interrupted(_)) => {
                return Err(LinkError::Io(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "websocket handshake timed out",
                )));
            }
        };
        Ok(WsLink { ws, closed: None, _in: PhantomData, _out: PhantomData })
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
        if let Some(code) = self.closed {
            return Err(LinkError::Closed { code });
        }
        let text = serde_json::to_string(msg).map_err(|e| LinkError::Protocol(e.to_string()))?;
        match self.ws.send(Message::text(text)) {
            Ok(()) => Ok(()),
            Err(e) => Err(self.map_after_error(e)),
        }
    }

    fn recv(&mut self, timeout: Duration) -> Result<Option<In>, LinkError> {
        if let Some(code) = self.closed {
            return Err(LinkError::Closed { code });
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
                    let code = frame.map(|f| u16::from(f.code));
                    self.closed = Some(code);
                    return Err(LinkError::Closed { code });
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
                self.closed = Some(None);
                LinkError::Closed { code: None }
            }
            tungstenite::Error::Io(io) => LinkError::Io(std::io::Error::new(io.kind(), io.to_string())),
            _ => LinkError::Protocol(e.to_string()),
        }
    }
}

/// resolve 된 주소를 차례로 [`CONNECT_TIMEOUT`] 씩 시도한다. 모두 실패하면 마지막 오류.
fn connect_tcp(host: &str, port: u16) -> Result<TcpStream, LinkError> {
    let mut last_err = std::io::Error::new(std::io::ErrorKind::NotFound, "host resolved to no address");
    for addr in (host, port).to_socket_addrs().map_err(LinkError::Io)? {
        match TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT) {
            Ok(stream) => return Ok(stream),
            Err(e) => last_err = e,
        }
    }
    Err(LinkError::Io(last_err))
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
/// 직접 문자열을 잘라 authority/host 를 뽑으면 `ws://localhost:1@evil.com/x` 처럼
/// userinfo 뒤에 진짜 host 를 숨기는 URL 을 통과시킬 수 있으므로, `tungstenite::http::Uri`
/// (tungstenite 가 `pub use http;` 로 재노출) 로 한 번만 파싱해서 판단한다. 파싱 실패는
/// `InsecureUrl` 로 막는다(fail closed).
/// 통과하면 파싱한 `Uri` 를 돌려주어 connect 가 같은 값으로 접속하게 한다.
fn ensure_secure_url(url: &str) -> Result<tungstenite::http::Uri, LinkError> {
    let uri: tungstenite::http::Uri = url.parse().map_err(|_| LinkError::InsecureUrl)?;

    let scheme = uri.scheme_str().unwrap_or("");
    if scheme.eq_ignore_ascii_case("wss") {
        return Ok(uri);
    }
    if !scheme.eq_ignore_ascii_case("ws") {
        return Err(LinkError::InsecureUrl);
    }

    let authority = uri.authority().ok_or(LinkError::InsecureUrl)?;
    // userinfo(`user:pass@host`)가 있으면 무조건 거부한다. `Authority::host()` 는 이미
    // userinfo 뒤의 진짜 host 를 돌려주지만, 애초에 userinfo 가 있는 ws:// URL 을 받아줄
    // 이유가 없으므로 검사를 명시적으로 둔다.
    let raw_authority = authority.as_str();
    if raw_authority.contains('@') {
        return Err(LinkError::InsecureUrl);
    }

    // `Authority::host()` 는 `[::1]evil` 처럼 IP-literal 뒤에 쓰레기가 붙어도 첫 `]` 에서
    // 잘라 host 만 돌려준다(예: `"[::1]"`). tungstenite 가 실제로 접속할 host 도 같은
    // 함수로 뽑으므로 값 자체는 tungstenite 와 일치하지만, host 뒤에 `:port` 가 아닌
    // 다른 글자가 남아있다면 URL 이 우리가 생각하는 것과 다른 모양이므로 거부한다.
    let raw_host = authority.host();
    match raw_authority.strip_prefix(raw_host) {
        Some(rest) if rest.is_empty() || rest.starts_with(':') => {}
        _ => return Err(LinkError::InsecureUrl),
    }

    // `Authority::host()` 는 IPv6 literal 이면 대괄호를 포함해서 돌려준다(`"[::1]"`).
    let host = strip_ipv6_brackets(raw_host);
    if host.eq_ignore_ascii_case("127.0.0.1") || host == "::1" || host.eq_ignore_ascii_case("localhost") {
        Ok(uri)
    } else {
        Err(LinkError::InsecureUrl)
    }
}

fn strip_ipv6_brackets(host: &str) -> &str {
    host.strip_prefix('[').and_then(|h| h.strip_suffix(']')).unwrap_or(host)
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

    #[test]
    fn uppercase_scheme_is_still_checked_correctly() {
        assert!(ensure_secure_url("WS://127.0.0.1:8787/x").is_ok());
        assert!(ensure_secure_url("WSS://example.com/x").is_ok());
    }

    /// userinfo 뒤에 진짜 host 를 숨겨 loopback 검사를 피해가려는 URL 들.
    /// `ws://localhost:8787@evil.com/x` 는 `localhost:8787` 이 userinfo 이고 진짜
    /// host 는 `evil.com` 이다 — 문자열을 손으로 잘라 첫 `:` 앞만 보면 `localhost` 로
    /// 착각해 통과시키는 버그가 났던 자리.
    #[test]
    fn userinfo_cannot_forge_a_loopback_host() {
        for url in [
            "ws://localhost:8787@evil.com/x",
            "ws://127.0.0.1:1@evil.com/x",
            "ws://localhost:1@evil.com/x",
            "ws://127.0.0.1@evil.com/x",
        ] {
            assert!(
                matches!(ensure_secure_url(url), Err(LinkError::InsecureUrl)),
                "userinfo 로 host 를 숨긴 url 은 거부돼야 한다: {url}"
            );
        }
    }

    #[test]
    fn lookalike_hosts_are_insecure() {
        for url in ["ws://127.0.0.1.evil.com/x", "ws://[::1]evil/x"] {
            assert!(
                matches!(ensure_secure_url(url), Err(LinkError::InsecureUrl)),
                "loopback 을 흉내낸 host 는 거부돼야 한다: {url}"
            );
        }
    }
}
