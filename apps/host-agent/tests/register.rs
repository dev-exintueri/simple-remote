//! `register_host` 를 순서가 정해진 가짜 signaling 서버에 붙여, 어긋나는 응답마다 막히는지 확인한다.

use std::net::TcpListener;
use std::thread::{self, JoinHandle};

use auth::DeviceKeys;
use ed25519_dalek::{Signature, VerifyingKey};
use host_agent::{RegisterError, register_host};
use transport::LinkError;
use tungstenite::Message;
use tungstenite::handshake::server::{Request, Response};

const NONCE: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
const ID: &str = "123456789";

enum Step {
    /// 이 JSON 을 보낸다.
    Send(String),
    /// client 가 보낸 text 하나를 받아 기록한다.
    Recv,
    /// close frame 을 보낸다.
    Close,
}

/// 서버가 본 것: 요청 경로(query 포함)와 client 가 보낸 text 들.
struct Seen {
    path: String,
    received: Vec<String>,
}

/// `steps` 를 차례로 실행하고, 끝나면 client 가 닫을 때까지 받은 text 를 기록하는 서버.
fn server(steps: Vec<Step>) -> (String, JoinHandle<Seen>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("ws://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let handle = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut path = String::new();
        let mut ws = tungstenite::accept_hdr(stream, |req: &Request, resp: Response| {
            path = req.uri().to_string();
            Ok(resp)
        })
        .unwrap();
        let mut received = Vec::new();
        for step in steps {
            match step {
                Step::Send(json) => ws.send(Message::text(json)).unwrap(),
                Step::Recv => match ws.read() {
                    Ok(Message::Text(t)) => received.push(t.to_string()),
                    other => panic!("expected a text frame, got {other:?}"),
                },
                Step::Close => {
                    let _ = ws.close(None);
                }
            }
        }
        loop {
            match ws.read() {
                Ok(Message::Text(t)) => received.push(t.to_string()),
                Ok(_) => {}
                Err(_) => break,
            }
        }
        Seen { path, received }
    });
    (base, handle)
}

fn challenge(nonce: &str, id: &str) -> Step {
    Step::Send(format!(r#"{{"t":"challenge","nonce":"{nonce}","id":"{id}"}}"#))
}

fn registered(id: &str) -> Step {
    Step::Send(format!(r#"{{"t":"registered","id":"{id}"}}"#))
}

fn keys() -> DeviceKeys {
    DeviceKeys::generate("host").unwrap()
}

/// `steps` 로 새 등록을 시도하고, 등록 결과와 서버가 본 것을 돌려준다.
fn run(steps: Vec<Step>, keys: &DeviceKeys, id: Option<&str>) -> (Result<String, RegisterError>, Seen) {
    let (base, handle) = server(steps);
    let result = register_host(&base, keys, id).map(|(id, _link)| id);
    (result, handle.join().unwrap())
}

#[test]
fn new_registration_signs_prefix_nonce_and_id() {
    let keys = keys();
    let (result, seen) = run(vec![challenge(NONCE, ID), Step::Recv, registered(ID)], &keys, None);
    assert_eq!(result.unwrap(), ID);
    assert_eq!(seen.path, format!("/v1/host?key={}", hex::encode(keys.device_public())));

    assert_eq!(seen.received.len(), 1);
    let auth: serde_json::Value = serde_json::from_str(&seen.received[0]).unwrap();
    assert_eq!(auth["t"], "auth");
    let sig: [u8; 64] = hex::decode(auth["sig"].as_str().unwrap()).unwrap().try_into().unwrap();
    let mut signed = b"simple-remote signaling auth v1".to_vec();
    signed.extend_from_slice(&hex::decode(NONCE).unwrap());
    signed.extend_from_slice(ID.as_bytes());
    VerifyingKey::from_bytes(&keys.device_public())
        .unwrap()
        .verify_strict(&signed, &Signature::from_bytes(&sig))
        .expect("signature over prefix || nonce || id");
}

fn assert_protocol_error_without_auth(steps: Vec<Step>, id: Option<&str>) -> Seen {
    let (result, seen) = run(steps, &keys(), id);
    assert!(matches!(result, Err(RegisterError::Protocol(_))), "got {result:?}");
    assert!(seen.received.is_empty(), "auth must not be sent: {:?}", seen.received);
    seen
}

#[test]
fn short_nonce_is_rejected_before_signing() {
    assert_protocol_error_without_auth(vec![challenge(&NONCE[..62], ID)], None);
}

#[test]
fn non_hex_nonce_is_rejected_before_signing() {
    let bad = format!("zz{}", &NONCE[2..]);
    assert_protocol_error_without_auth(vec![challenge(&bad, ID)], None);
}

#[test]
fn challenge_id_must_be_nine_digits() {
    assert_protocol_error_without_auth(vec![challenge(NONCE, "12345678a")], None);
    assert_protocol_error_without_auth(vec![challenge(NONCE, "12345678")], None);
}

#[test]
fn registered_id_must_match_challenge_id() {
    let (result, seen) = run(vec![challenge(NONCE, ID), Step::Recv, registered("987654321")], &keys(), None);
    assert!(matches!(result, Err(RegisterError::Protocol(_))), "got {result:?}");
    assert_eq!(seen.received.len(), 1);
}

#[test]
fn re_registration_uses_id_path_and_rejects_other_challenge_id() {
    let keys = keys();
    let seen = assert_protocol_error_without_auth(vec![challenge(NONCE, "987654321")], Some(ID));
    // 경로의 key 는 이 테스트의 key 와 다르므로 모양만 확인한다.
    assert!(seen.path.starts_with(&format!("/v1/host/{ID}?key=")), "path {}", seen.path);
    assert_eq!(seen.path.len(), format!("/v1/host/{ID}?key=").len() + 64);

    let (result, seen) = run(vec![challenge(NONCE, ID), Step::Recv, registered(ID)], &keys, Some(ID));
    assert_eq!(result.unwrap(), ID);
    assert_eq!(seen.path, format!("/v1/host/{ID}?key={}", hex::encode(keys.device_public())));
}

#[test]
fn unexpected_first_message_is_rejected() {
    assert_protocol_error_without_auth(vec![Step::Send(r#"{"t":"viewer_joined"}"#.into())], None);
}

#[test]
fn close_before_registered_is_a_link_error() {
    let (result, seen) = run(vec![challenge(NONCE, ID), Step::Recv, Step::Close], &keys(), None);
    assert!(matches!(result, Err(RegisterError::Link(LinkError::Closed { .. }))), "got {result:?}");
    assert_eq!(seen.received.len(), 1);
}
