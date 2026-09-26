use std::io::Write as _;
use std::net::TcpListener;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use protocol::signaling::{ServerToViewer, ViewerToServer};
use transport::{Link, LinkError, WsLink};

/// 받은 text frame 을 그대로 돌려주는 서버. `ws://127.0.0.1:<port>/...` url 을 돌려준다.
fn echo_server() -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let url = format!("ws://127.0.0.1:{port}/v1/viewer/1");

    let handle = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut ws = tungstenite::accept(stream).unwrap();
        loop {
            match ws.read() {
                Ok(tungstenite::Message::Text(text)) => {
                    if ws.send(tungstenite::Message::Text(text)).is_err() {
                        break;
                    }
                }
                Ok(tungstenite::Message::Close(_)) => break,
                Err(_) => break,
                _ => {}
            }
        }
    });

    (url, handle)
}

/// 업그레이드 없이 404 를 쓰고 닫는 서버.
fn http_404_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let url = format!("ws://127.0.0.1:{port}/x");

    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
    });

    url
}

#[test]
fn ws_roundtrip_and_timeout() {
    let (url, server) = echo_server();
    let mut l: WsLink<ServerToViewer, ViewerToServer> = WsLink::connect(&url).unwrap();
    assert!(l.recv(Duration::from_millis(100)).unwrap().is_none());
    l.send(&ViewerToServer::Relay { data: "abcd".into() }).unwrap();
    assert_eq!(
        l.recv(Duration::from_secs(2)).unwrap(),
        Some(ServerToViewer::Relay { data: "abcd".into() })
    );
    drop(l);
    server.join().unwrap();
}

#[test]
fn http_error_before_upgrade() {
    let url = http_404_server();
    assert!(matches!(
        WsLink::<ServerToViewer, ViewerToServer>::connect(&url),
        Err(LinkError::Http { status: 404 })
    ));
}

#[test]
fn plain_ws_only_on_loopback() {
    assert!(matches!(
        WsLink::<ServerToViewer, ViewerToServer>::connect("ws://example.com/v1/viewer/1"),
        Err(LinkError::InsecureUrl)
    ));
    assert!(matches!(
        WsLink::<ServerToViewer, ViewerToServer>::connect("http://127.0.0.1/x"),
        Err(LinkError::InsecureUrl)
    ));
}

#[test]
fn plain_ws_allowed_on_named_loopback_forms() {
    // 실제로 연결하지 않고 url 검사만 통과하는지 본다: 연결이 거부되면 InsecureUrl 이 아닌
    // 다른 오류(Io 등)가 나야 한다.
    match WsLink::<ServerToViewer, ViewerToServer>::connect("ws://localhost:8787/x") {
        Err(LinkError::InsecureUrl) => panic!("localhost 는 허용돼야 한다"),
        _ => {}
    }
    match WsLink::<ServerToViewer, ViewerToServer>::connect("ws://[::1]:8787/x") {
        Err(LinkError::InsecureUrl) => panic!("[::1] 는 허용돼야 한다"),
        _ => {}
    }
}

/// TCP 연결은 받지만 HTTP upgrade 에 답하지 않는 서버: 연결 대기에 상한이 있어야 한다.
#[test]
fn silent_server_does_not_hang_connect() {
    // accept 하지 않아도 kernel 이 TCP 연결을 끝내므로, listener 를 살려 두기만 한다.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let url = format!("ws://127.0.0.1:{port}/v1/viewer/1");

    let started = std::time::Instant::now();
    let result = WsLink::<ServerToViewer, ViewerToServer>::connect(&url);
    let elapsed = started.elapsed();

    assert!(result.is_err(), "upgrade 응답이 없으면 오류여야 한다");
    assert!(elapsed < Duration::from_secs(12), "연결 대기가 너무 길다: {elapsed:?}");
    drop(listener);
}

/// Worker 는 64 KiB 넘는 메시지를 받지 않으므로, 그보다 훨씬 큰 메시지는 전달하지 않고 오류로 낸다.
#[test]
fn oversized_message_is_rejected() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let url = format!("ws://127.0.0.1:{port}/v1/viewer/1");

    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut ws = tungstenite::accept(stream).unwrap();
        let big = format!(r#"{{"t":"relay","data":"{}"}}"#, "a".repeat(200 * 1024));
        let _ = ws.send(tungstenite::Message::text(big));
        // client 가 읽을 때까지 socket 을 열어 둔다.
        let _ = ws.read();
    });

    let mut l: WsLink<ServerToViewer, ViewerToServer> = WsLink::connect(&url).unwrap();
    let got = l.recv(Duration::from_secs(5));
    assert!(matches!(got, Err(LinkError::Protocol(_))), "큰 메시지는 Protocol 오류여야 한다: {got:?}");
    drop(l);
    server.join().unwrap();
}
