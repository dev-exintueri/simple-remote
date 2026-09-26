# SP1 연결 뼈대 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** viewer 와 host-agent 가 signaling 서버를 거쳐 6자리 코드 PAKE → PAKE key 로 보호한 SDP 교환 → str0m 직접 연결 → 기기 key 교환·서명 증명 → 허락 → data channel ping/pong 까지 영상 없이 끝까지 동작하게 한다.

**Architecture:** 보안·메시지 규칙은 IO 없는 순수 코드(`crates/protocol`, `crates/auth`, host 시도 제한)로 만들어 단위 테스트하고, 네트워크는 `crates/transport` 의 str0m+UDP `Peer` 와 signaling `Link` trait 뒤에 둔다. 연결 흐름(`viewer-core::connect`, `host_agent::serve_next`)은 `Link` 의 메모리 가짜 구현과 loopback UDP 로 한 프로세스 안에서 통합 테스트하고, 마지막에 로컬 `wrangler dev` signaling 을 거치는 end-to-end script 로 확인한다. 모든 IO 는 동기(thread + timeout)로 하고 async runtime 은 쓰지 않는다.

**Tech Stack:** Rust 1.95 (edition 2024), `str0m =0.23.1`, `pakery-spake2 =0.6.0`, `ed25519-dalek 3.0.0`, `x25519-dalek 3.0.0`, `chacha20poly1305 0.11.0`, `hkdf 0.13.0` + `sha2 0.11.0`, `getrandom 0.4.3`, `rand_core 0.10.1`, `serde`/`serde_json`/`postcard 1.1.3`, `tungstenite 0.30.0` + `rustls 0.23`(ring), TypeScript Cloudflare Worker + Durable Object (`wrangler 4.137.0`, `@cloudflare/vitest-plugin 1.2.4`, `vitest 4.1.11`).

**Spec:** `docs/superpowers/specs/2026-09-24-simple-remote-sp1-design.md` (5.1, 5.2, 5.5, 8.1, 3.2, 4, 7.5, 9.1). 근거: `docs/research/2026-09-23-sp1-phase0-spike-results.md` (T2, T3, T4, T7, T8).

## 이 계획의 범위

- 포함: Rust workspace, `crates/protocol`, `crates/auth`(기기 key, 재접속 공개키 서명 묶음, PAKE, 봉인 채널, 일회용 코드), `crates/transport`(str0m `Peer`, signaling `Link`), `crates/viewer-core`(첫 연결), `apps/host-agent`(등록, 시도 제한, 첫 연결 응답, 콘솔 실행 파일), `apps/viewer`(콘솔 실행 파일), `signaling/`(ID 발급·key 소유 증명, host 대기, viewer 연결, 중계, 요청 제한), Linux CI, 로컬 end-to-end script.
- 다음 계획으로 미룸 (이 계획의 코드에 자리만 만들지 않는다): 재접속 허가증과 Noise KK(5.3), 기기 key·ID 디스크 저장(DPAPI), STUN·공유기 매핑·경로 기록(5.4), heartbeat 단계 대응·ICE restart(8.3), host-ui 허락 창(콘솔 질문으로 대신), 접속 기록(5.6), 영상·입력·클립보드, Windows 서비스 구조.
- 이 계획의 실행 파일은 콘솔 데모다. host-agent 는 실행할 때마다 새 기기 key 를 만들어 새 ID 를 받는다 (저장은 다음 계획).

## Global Constraints

- Rust toolchain `1.95` (`rust-toolchain.toml`), edition 2024. Windows 에서도 빌드되는 코드만 쓴다 (Unix 전용 API 금지).
- 의존성 세대: `rand_core` 0.10 / `digest` 0.11 / `sha2` 0.11 계열만 쓴다. `rand_core` 0.6 `OsRng` 는 쓰지 않는다. 난수는 `rand_core::UnwrapErr(getrandom::SysRng)` 하나로 만든다 (`getrandom` feature `sys_rng`).
- `str0m = { version = "=0.23.1", default-features = false, features = ["aws-lc-rs"] }` (D23). `pakery-spake2`, `pakery-core`, `pakery-crypto` 는 `=0.6.0` 고정 (D25).
- PAKE 를 가장 먼저 하고, key 확인 MAC 이 양쪽에서 끝나기 전에는 주소 후보(SDP)를 보내지 않는다. SDP 는 PAKE key 로 AEAD 봉인한 채로만 signaling 을 지난다 (spec 5.2 3~4단계).
- 코드는 숫자 6자리 일회용, 접속 ID 는 숫자 9자리이고 signaling 서버가 발급해 host 공개키에 묶는다. host 는 등록마다 서명으로 key 소유를 증명한다 (spec 4절).
- 코드 추측 대비: PAKE 실패마다 다음 시도까지 대기 시간을 2배로 늘린다 (1초부터). 한 코드에 5번 틀리면 코드를 교체한다. 대기 시간은 코드를 교체해도 유지하고 PAKE 가 성공해야 초기화한다 (spec 5.5).
- 허락은 30초 안에 하지 않으면 거절. 허락되면 사용한 코드는 폐기하고 새 코드를 만든다 (spec 5.2 6~7단계).
- 보안 경로는 실패하면 막는다 (spec 8.1): PAKE 실패·MAC 불일치, 기기 key 서명 확인 실패, DTLS 지문 불일치, 허락 무응답·거절, protocol 버전 불호환이면 연결을 끊는다. 암호화 없이 진행하는 코드 경로를 두지 않는다.
- signaling 연결은 `wss://` 만 쓴다 (spec 5.1). 예외: 개발용 loopback 주소(`127.0.0.1`, `::1`, `localhost`)의 `ws://`.
- 코드, key, 봉인 전 SDP 는 로그와 표준 출력에 남기지 않는다 (spec 7.6). 예외: host 콘솔 데모가 가족에게 보여 줄 코드를 출력하는 한 줄 (host-ui 의 코드 표시 역할).
- signaling 은 Cloudflare Workers 무료 plan 의 SQLite 기반 Durable Object 만 쓴다 (`new_sqlite_classes`).
- 값 중 spec 에 없는 것은 "설계 제안값"으로 표시했다: 단계별 signaling 대기 15초, 직접 연결 대기 10초, 요청 제한 수치, 메시지 크기 상한, 기기 이름 64 byte 상한, 대기 시간 상한 1일.

## Review Focus

1. **host 가 중간에 사라짐**: PAKE 도중 host 연결이 끊기면 viewer 는 15초 안에 `ConnectError::HostLeft` 를 받아야 한다 (멈추지 않음). → Task 9 `host_leaves_mid_handshake`.
2. **악성 signaling 이 봉인 메시지를 바꾸거나 다시 보냄**: 변조·재전송·순서 바뀜은 모두 거부되고 연결이 끊겨야 한다. → Task 4 `replay_rejected`, `reorder_rejected`, `tamper_rejected`; Task 9 `garbage_confirm_counts_as_failure`.
3. **대기 시간 중 재시도**: host 가 대기 중일 때 온 viewer 는 PAKE 없이 남은 시간을 받아야 한다. → Task 9 `retry_after_during_backoff`.
4. **DTLS 지문 바꿔치기**: SDP 의 지문과 실제 인증서가 다르면 data channel 이 열리지 않아야 한다. → Task 6 `fingerprint_mismatch_never_opens`.
5. **CF-Connecting-IP 없는 요청과 잘못된 길이의 서명**: signaling 은 거부해야 한다 (요청 제한 우회·예외 방지). → Task 5 `rejects_missing_client_ip`, `rejects_short_signature`.

---

## 파일 구조

```
Cargo.toml                      workspace, 공용 의존성 버전
rust-toolchain.toml             1.95
.gitignore                      target, node_modules, .wrangler
.github/workflows/ci.yml        Linux: cargo test, signaling npm test
crates/protocol/src/
  lib.rs                        PROTOCOL_VERSION, Timeouts, 재수출
  control.rs                    data channel 메시지 (Control, Hello, Decision)
  pake_msg.rs                   signaling 중계로 오가는 PAKE·SDP 메시지 (PakeMsg)
  signaling.rs                  signaling 서버와의 JSON 메시지
crates/auth/src/
  lib.rs, error.rs, rng.rs
  identity.rs                   DeviceKeys, Hello 만들기·확인, key 지문
  code.rs                       OneTimeCode
  pake.rs                       ViewerPake, HostPake, SessionKeys
  sealed.rs                     SealedSender, SealedReceiver
crates/auth/tests/rfc9382.rs    RFC 9382 vector 고정
crates/transport/src/
  lib.rs, error.rs
  net.rs                        local_ips()
  peer.rs                       Peer (str0m + UDP socket)
  link.rs                       Link trait, LinkError, WsLink
  fake_signal.rs                FakeHub (feature test-support)
crates/transport/tests/peer.rs, tests/ws_link.rs
crates/viewer-core/src/
  lib.rs, connect.rs            connect(), ViewerSession, ConnectError
apps/host-agent/src/
  lib.rs, attempts.rs           AttemptLimiter
  session.rs                    serve_next(), HostSession, HostState
  register.rs                   register_host()
  main.rs                       콘솔 데모
apps/host-agent/tests/first_connection.rs
apps/viewer/src/main.rs         콘솔 데모
signaling/                      package.json, wrangler.jsonc, vitest.config.ts, tsconfig.json,
                                worker-configuration.d.ts, src/{index,host-room,rate-limit,hex,ed25519}.ts,
                                test/{rate-limit,register,relay}.test.ts
tools/e2e-local.sh              wrangler dev + host-agent + viewer 로 끝까지 확인
```

---

### Task 1: workspace 뼈대, `crates/protocol`, Linux CI

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `.gitignore`, `.github/workflows/ci.yml`
- Create: `crates/protocol/Cargo.toml`, `crates/protocol/src/{lib.rs,control.rs,pake_msg.rs,signaling.rs}`

**Interfaces:**
- Produces:
  - `protocol::PROTOCOL_VERSION: u32 = 1`
  - `protocol::Timeouts { pub signaling_step: Duration, pub connect: Duration, pub approval: Duration }`, `impl Default` = 15초 / 10초 / 30초
  - `protocol::Stage { Join, Pake, Answer, Connect, Hello, Decision, Ping }` (시간 초과가 난 단계. viewer 와 host 가 함께 씀)
  - `protocol::control::{Control, Hello, Decision, RejectReason, encode, decode, MAX_CONTROL_BYTES}`
    - `enum Control { Hello(Hello), Decision(Decision), Ping { seq: u64 }, Pong { seq: u64 } }`
    - `struct Hello { pub protocol_version: u32, pub device_key: Vec<u8>, pub device_name: String, pub reconnect_key: Vec<u8>, pub reconnect_key_sig: Vec<u8>, pub session_sig: Vec<u8> }` (serde 가 `[u8; 64]` 를 derive 하지 못하므로 `Vec<u8>` 로 두고 길이는 `auth` 가 확인)
    - `enum Decision { Accepted, Rejected(RejectReason) }`, `enum RejectReason { Denied, NoResponse, VersionMismatch { host_version: u32 }, BadIdentity }`
    - `fn encode(msg: &Control) -> Vec<u8>`, `fn decode(bytes: &[u8]) -> Result<Control, DecodeError>`; `MAX_CONTROL_BYTES = 16 * 1024` (설계 제안값)
  - `protocol::pake_msg::{PakeMsg, encode, decode}`: `enum PakeMsg { Start { pa: Vec<u8> }, Reply { pb: Vec<u8>, mac_b: Vec<u8> }, Confirm { mac_a: Vec<u8>, sealed_offer: Vec<u8> }, Answer { sealed_answer: Vec<u8> }, RetryAfter { ms: u64 }, Rejected }`
  - `protocol::signaling::{ServerToHost, HostToServer, ServerToViewer, ViewerToServer}` (serde `tag = "t"`, `rename_all = "snake_case"`, 이진 값은 소문자 hex 문자열)
    - `ServerToHost { Challenge { nonce: String, id: String }, Registered { id: String }, ViewerJoined, ViewerLeft, Relay { data: String } }` (새 등록은 host 가 서명 전에 ID 를 모르므로 challenge 에 서버가 고른 ID 를 싣는다)
    - `HostToServer { Auth { sig: String }, Relay { data: String }, Kick }`
    - `ServerToViewer { Joined, Relay { data: String }, HostLeft }`
    - `ViewerToServer { Relay { data: String } }`
  - 공용 오류 `protocol::DecodeError` (thiserror)

- [ ] **Step 1: workspace 파일 작성**

`Cargo.toml` 은 `[workspace] resolver = "3"`, `members = ["crates/*", "apps/*"]`, `[workspace.package] edition = "2024"`, `[workspace.dependencies]` 에 Tech Stack 의 버전을 모두 적는다:

```toml
[workspace.dependencies]
protocol = { path = "crates/protocol" }
auth = { path = "crates/auth" }
transport = { path = "crates/transport" }
viewer-core = { path = "crates/viewer-core" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
postcard = { version = "1.1.3", features = ["use-std"] }
thiserror = "2.0.21"
hex = "0.4"
zeroize = "1.9.0"
rand_core = "0.10.1"
getrandom = { version = "0.4.3", features = ["sys_rng"] }
ed25519-dalek = { version = "3.0.0", features = ["rand_core"] }
x25519-dalek = { version = "3.0.0", features = ["static_secrets"] }
chacha20poly1305 = "0.11.0"
hkdf = "0.13.0"
sha2 = "0.11.0"
pakery-spake2 = "=0.6.0"
pakery-core = "=0.6.0"
pakery-crypto = { version = "=0.6.0", default-features = false, features = ["std", "p256", "spake2"] }
str0m = { version = "=0.23.1", default-features = false, features = ["aws-lc-rs"] }
tungstenite = { version = "0.30.0", features = ["rustls-tls-webpki-roots"] }
rustls = { version = "0.23", default-features = false, features = ["std", "ring"] }
```

`rust-toolchain.toml`: `[toolchain] channel = "1.95"`. `.gitignore`: `/target`, `signaling/node_modules`, `signaling/.wrangler`.

- [ ] **Step 2: 실패하는 테스트 작성** — `crates/protocol/src/control.rs`, `pake_msg.rs`, `signaling.rs` 각각의 `#[cfg(test)] mod tests`:

```rust
// control.rs
#[test]
fn control_roundtrip() {
    let msg = Control::Hello(Hello {
        protocol_version: 1, device_key: vec![1; 32], device_name: "노트북".into(),
        reconnect_key: vec![2; 32], reconnect_key_sig: vec![3; 64], session_sig: vec![4; 64],
    });
    assert_eq!(decode(&encode(&msg)).unwrap(), msg);
    let d = Control::Decision(Decision::Rejected(RejectReason::VersionMismatch { host_version: 7 }));
    assert_eq!(decode(&encode(&d)).unwrap(), d);
}
#[test]
fn decode_rejects_garbage_and_oversize() {
    assert!(decode(&[0xff, 0xff, 0xff]).is_err());
    assert!(decode(&vec![0u8; MAX_CONTROL_BYTES + 1]).is_err());
}

// pake_msg.rs
#[test]
fn pake_msg_roundtrip() {
    for m in [PakeMsg::Start { pa: vec![4; 65] }, PakeMsg::RetryAfter { ms: 2000 }, PakeMsg::Rejected] {
        assert_eq!(decode(&encode(&m)).unwrap(), m);
    }
}

// signaling.rs: TypeScript 쪽과 맞출 정확한 JSON
#[test]
fn signaling_json_shapes() {
    assert_eq!(serde_json::to_string(&HostToServer::Kick).unwrap(), r#"{"t":"kick"}"#);
    assert_eq!(serde_json::to_string(&HostToServer::Auth { sig: "ab".into() }).unwrap(), r#"{"t":"auth","sig":"ab"}"#);
    assert_eq!(serde_json::to_string(&ViewerToServer::Relay { data: "00".into() }).unwrap(), r#"{"t":"relay","data":"00"}"#);
    let m: ServerToHost = serde_json::from_str(r#"{"t":"viewer_joined"}"#).unwrap();
    assert_eq!(m, ServerToHost::ViewerJoined);
    let m: ServerToViewer = serde_json::from_str(r#"{"t":"host_left"}"#).unwrap();
    assert_eq!(m, ServerToViewer::HostLeft);
    let m: ServerToHost = serde_json::from_str(r#"{"t":"registered","id":"123456789"}"#).unwrap();
    assert_eq!(m, ServerToHost::Registered { id: "123456789".into() });
    let m: ServerToHost = serde_json::from_str(r#"{"t":"challenge","nonce":"00ff","id":"123456789"}"#).unwrap();
    assert_eq!(m, ServerToHost::Challenge { nonce: "00ff".into(), id: "123456789".into() });
}
```

- [ ] **Step 3: 실패 확인** — `cargo test -p protocol`. 기대: 타입이 없어 컴파일 실패.
- [ ] **Step 4: 구현** — 위 Interfaces 의 타입(`#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]`)과 `encode`/`decode`(postcard `to_allocvec`/`from_bytes`, `decode` 는 길이 상한을 먼저 확인)를 만든다. `Timeouts` 는 `lib.rs`.
- [ ] **Step 5: 통과 확인** — `cargo test -p protocol`. 기대: 4 passed.
- [ ] **Step 6: CI 작성** — `.github/workflows/ci.yml`: `on: push (branches: [develop]), pull_request, workflow_dispatch`, `paths: ['crates/**','apps/**','signaling/**','Cargo.toml','Cargo.lock','rust-toolchain.toml','.github/workflows/ci.yml']`. job `rust` (ubuntu-latest): `actions/checkout@v7`, `rustup toolchain install 1.95 --profile minimal`, `cargo test --workspace --all-features`. job `signaling` (ubuntu-latest, Task 5 에서 채움): `actions/setup-node` Node 22, `npm install -g npm@11`, `npm ci`, `npm run typecheck`, `npm test` (working-directory `signaling`, `if: hashFiles('signaling/package.json') != ''`). Linux 는 무료 분 차감 1배라 Windows workflow 규칙(D24)과 따로 둔다.
- [ ] **Step 7: commit** — `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.gitignore`, `.github/workflows/ci.yml`, `crates/protocol/**` 를 하나씩 add. 메시지 "feat: Rust workspace 와 protocol 메시지 정의".

---

### Task 2: `crates/auth` 기기 key 와 Hello 서명

**Files:**
- Create: `crates/auth/Cargo.toml`, `crates/auth/src/{lib.rs,error.rs,rng.rs,identity.rs}`

**Interfaces:**
- Consumes: `protocol::control::Hello`, `protocol::PROTOCOL_VERSION`
- Produces:
  - `auth::rng() -> rand_core::UnwrapErr<getrandom::SysRng>`
  - `auth::AuthError` (thiserror): `WrongCode`, `Malformed(&'static str)`, `BadSignature`, `BadSeal`, `NameTooLong`
  - `auth::Role { Viewer, Host }` (`Role::byte()`: Viewer=1, Host=2)
  - `auth::DeviceKeys`: `generate(name: &str) -> Result<DeviceKeys, AuthError>` (이름은 1~64 byte UTF-8, 설계 제안값), `name(&self) -> &str`, `device_public(&self) -> [u8; 32]`, `reconnect_public(&self) -> [u8; 32]`, `sign_raw(&self, msg: &[u8]) -> [u8; 64]`, `make_hello(&self, role: Role, binding: &[u8; 32], own_fp: &[u8], peer_fp: &[u8]) -> Hello`
  - `auth::PeerIdentity { pub device_key: [u8; 32], pub name: String, pub reconnect_key: [u8; 32] }`
  - `auth::verify_hello(hello: &Hello, sender: Role, binding: &[u8; 32], sender_fp: &[u8], receiver_fp: &[u8]) -> Result<PeerIdentity, AuthError>` (버전은 확인하지 않는다. 버전 판단은 연결 흐름 담당)
  - `auth::key_fingerprint(key: &[u8; 32]) -> String`: SHA-256 앞 8 byte 를 4자리 hex 묶음 4개로 (`"a1b2 c3d4 e5f6 0718"`). 허락 질문과 로그에 쓰는 표시용.
- 서명 규칙 (정확한 byte):
  - 재접속 key 묶음: `reconnect_key_sig = Sign(device, b"simple-remote reconnect key v1" || reconnect_public)`. X25519 재접속 key 는 이번 계획에서 교환만 하고 쓰지 않는다 (D26, 다음 계획의 Noise KK 가 쓴다).
  - 세션 서명: `session_sig = Sign(device, b"simple-remote hello v1" || [role.byte()] || binding || u16_be(own_fp.len()) || own_fp || u16_be(peer_fp.len()) || peer_fp || u16_be(name.len()) || name(UTF-8))`. 기기 이름은 허락 창에 보이므로 서명에 넣어 바꿔치기를 막는다. `binding` 은 Task 3 `SessionKeys::hello_binding` 이고 fp 는 DTLS 인증서 지문 byte (Task 6 `Peer::local_fingerprint`/`remote_fingerprint`).
  - 확인은 `VerifyingKey::verify_strict`. 길이가 틀린 key·서명은 `Malformed`.

- [ ] **Step 1: 실패하는 테스트 작성** — `identity.rs` 의 tests:

```rust
fn setup() -> (DeviceKeys, [u8; 32], Vec<u8>, Vec<u8>) {
    (DeviceKeys::generate("내 노트북").unwrap(), [9u8; 32], vec![1; 32], vec![2; 32])
}
#[test]
fn hello_roundtrip() {
    let (k, b, vfp, hfp) = setup();
    let h = k.make_hello(Role::Viewer, &b, &vfp, &hfp);
    let id = verify_hello(&h, Role::Viewer, &b, &vfp, &hfp).unwrap();
    assert_eq!(id.device_key, k.device_public());
    assert_eq!(id.reconnect_key, k.reconnect_public());
    assert_eq!(id.name, "내 노트북");
}
#[test]
fn hello_rejects_tampering() {
    let (k, b, vfp, hfp) = setup();
    let h = k.make_hello(Role::Viewer, &b, &vfp, &hfp);
    assert!(verify_hello(&h, Role::Host, &b, &vfp, &hfp).is_err());          // 역할 바꿔치기
    assert!(verify_hello(&h, Role::Viewer, &[8u8; 32], &vfp, &hfp).is_err()); // 다른 세션
    assert!(verify_hello(&h, Role::Viewer, &b, &hfp, &vfp).is_err());         // 지문 뒤바꿈
    let mut n = h.clone(); n.device_name = "다른 이름".into();
    assert!(verify_hello(&n, Role::Viewer, &b, &vfp, &hfp).is_err());
    let other = DeviceKeys::generate("x").unwrap();
    let mut r = h.clone(); r.reconnect_key = other.reconnect_public().to_vec();
    assert!(verify_hello(&r, Role::Viewer, &b, &vfp, &hfp).is_err());         // 남의 재접속 key
    let mut s = h.clone(); s.session_sig.truncate(63);
    assert!(matches!(verify_hello(&s, Role::Viewer, &b, &vfp, &hfp), Err(AuthError::Malformed(_))));
}
#[test]
fn name_limits_and_fingerprint_format() {
    assert!(matches!(DeviceKeys::generate(""), Err(AuthError::NameTooLong)));
    assert!(DeviceKeys::generate(&"가".repeat(22)).is_err()); // 66 byte
    let fp = key_fingerprint(&[0u8; 32]);
    assert_eq!(fp.len(), 19);
    assert_eq!(fp.split(' ').count(), 4);
}
```

- [ ] **Step 2: 실패 확인** — `cargo test -p auth`. 기대: 컴파일 실패.
- [ ] **Step 3: 구현** — `identity.rs`. `SigningKey::generate(&mut rng())`, `StaticSecret::random_from_rng(&mut rng())`, `use ed25519_dalek::Signer;`. 비밀 key 는 `zeroize` 된다 (`ed25519-dalek` 기본 feature `zeroize`, `StaticSecret` 은 drop 시 zeroize). `Debug` 는 비밀 값을 찍지 않게 직접 구현.
- [ ] **Step 4: 통과 확인** — `cargo test -p auth`. 기대: 3 passed.
- [ ] **Step 5: commit** — "feat: 기기 key 와 Hello 서명 확인".

---

### Task 3: `crates/auth` 일회용 코드, PAKE, key 일정

**Files:**
- Create: `crates/auth/src/{code.rs,pake.rs}`, `crates/auth/tests/rfc9382.rs`

**Interfaces:**
- Produces:
  - `auth::OneTimeCode`: `generate() -> OneTimeCode` (숫자 6자리 균등, `getrandom::fill` + 거부 표본), `parse(input: &str) -> Result<OneTimeCode, AuthError>` (공백 제거 후 정확히 숫자 6개), `as_str(&self) -> &str`. `Debug` 는 `"OneTimeCode(******)"`.
  - `auth::ViewerPake::start(code: &OneTimeCode, host_id: &str) -> Result<(ViewerPake, Vec<u8>), AuthError>` → `pa` (65 byte)
  - `ViewerPake::finish(self, pb: &[u8], mac_b: &[u8]) -> Result<(SessionKeys, Vec<u8>), AuthError>` → `mac_a`. host MAC 이 틀리면 `WrongCode`.
  - `auth::HostPake::respond(code: &OneTimeCode, host_id: &str, pa: &[u8]) -> Result<(HostPake, Vec<u8>, Vec<u8>), AuthError>` → `(pending, pb, mac_b)`
  - `HostPake::confirm(self, mac_a: &[u8]) -> Result<SessionKeys, AuthError>`. 틀리면 `WrongCode`.
  - `auth::SessionKeys { pub viewer_to_host: [u8; 32], pub host_to_viewer: [u8; 32], pub hello_binding: [u8; 32] }` (`ZeroizeOnDrop`)
- 정한 값 (정확한 byte):
  - 역할: viewer = SPAKE2 A, host = SPAKE2 B. suite `Spake2P256` (P256-SHA256-HKDF-SHA256-HMAC-SHA256).
  - `idA = b"simple-remote viewer"`, `idB = b"simple-remote host " || host_id`, `aad = b"simple-remote pake v1"`
  - `w = P256Group::scalar_from_wide_bytes(Sha512Hash::digest(b"simple-remote code v1\0" || code))` (spike T2 와 같은 변환에 문맥 문자열만 더함)
  - key 일정: `hk = Hkdf::<Sha256>::new(None, Ke)` (Ke 16 byte = `session_key`), `expand` info `b"simple-remote v1 seal viewer->host"`, `b"simple-remote v1 seal host->viewer"`, `b"simple-remote v1 hello binding"` 각 32 byte.
  - `pa`, `pb` 가 point 로 해석되지 않으면 `Malformed("pake point")`.

- [ ] **Step 1: 실패하는 테스트 작성**
  - `crates/auth/tests/rfc9382.rs`: `spikes/auth/tests/pakery_rfc9382.rs` 의 `c_rfc9382_vector1_full` 테스트를 상수와 함께 그대로 옮긴다 (spec 5.7: 고정 버전의 RFC 9382 동작을 제품 테스트로 잠근다). dev-dependency `hex`, `pakery-core`.
  - `pake.rs` tests:

```rust
fn code(s: &str) -> OneTimeCode { OneTimeCode::parse(s).unwrap() }
#[test]
fn same_code_agrees() {
    let (v, pa) = ViewerPake::start(&code("123456"), "123456789").unwrap();
    let (h, pb, mac_b) = HostPake::respond(&code("123456"), "123456789", &pa).unwrap();
    let (vk, mac_a) = v.finish(&pb, &mac_b).unwrap();
    let hk = h.confirm(&mac_a).unwrap();
    assert_eq!(vk.viewer_to_host, hk.viewer_to_host);
    assert_eq!(vk.hello_binding, hk.hello_binding);
    assert_ne!(vk.viewer_to_host, vk.host_to_viewer);
}
#[test]
fn wrong_code_fails_on_both_sides() {
    let (v, pa) = ViewerPake::start(&code("123456"), "123456789").unwrap();
    let (h, pb, mac_b) = HostPake::respond(&code("654321"), "123456789", &pa).unwrap();
    assert!(matches!(v.finish(&pb, &mac_b), Err(AuthError::WrongCode)));
    assert!(matches!(h.confirm(&[0u8; 32]), Err(AuthError::WrongCode)));
}
#[test]
fn host_id_is_bound() {
    let (v, pa) = ViewerPake::start(&code("123456"), "111111111").unwrap();
    let (_h, pb, mac_b) = HostPake::respond(&code("123456"), "222222222", &pa).unwrap();
    assert!(matches!(v.finish(&pb, &mac_b), Err(AuthError::WrongCode)));
}
#[test]
fn malformed_point_rejected() {
    assert!(matches!(HostPake::respond(&code("123456"), "123456789", &[4u8; 65]), Err(AuthError::Malformed(_))));
}
```

  - `code.rs` tests: `generate` 결과 200개가 모두 숫자 6자리이고 서로 다른 값이 2개 이상; `parse("123 456")` 은 `"123456"`; `parse("12345")`, `parse("1234567")`, `parse("12a456")` 는 오류; `format!("{:?}", code)` 에 숫자가 없음.
- [ ] **Step 2: 실패 확인** — `cargo test -p auth`. 기대: 새 타입이 없어 컴파일 실패.
- [ ] **Step 3: 구현** — `PartyA/PartyB<Spake2P256>::start(&w, idA, idB, aad, &mut rng())`, `finish`, `verify_peer_confirmation` (사용법은 `spikes/auth/tests/pakery_rfc9382.rs`). `Spake2Error::ConfirmationFailed` → `WrongCode`, 그 밖의 오류 → `Malformed("pake")`.
- [ ] **Step 4: 통과 확인** — `cargo test -p auth`. 기대: 모두 통과 (RFC vector 포함).
- [ ] **Step 5: commit** — "feat: 일회용 코드와 SPAKE2 key 합의".

---

### Task 4: `crates/auth` 봉인 채널

**Files:**
- Create: `crates/auth/src/sealed.rs`

**Interfaces:**
- Consumes: `SessionKeys` (Task 3)
- Produces:
  - `auth::SealedSender::seal(&mut self, plaintext: &[u8]) -> Vec<u8>`: 출력 = `u64_be(counter) || ChaCha20Poly1305(ciphertext+tag)`, nonce = `[0; 4] || u64_be(counter)`, AAD = 방향 label. counter 는 0 부터.
  - `auth::SealedReceiver::open(&mut self, msg: &[u8]) -> Result<Vec<u8>, AuthError>`: counter 가 기대값과 정확히 같을 때만 열고 기대값을 1 올린다. 실패하면 `BadSeal` 이고 상태는 그대로.
  - `SessionKeys::viewer_side(&self) -> (SealedSender, SealedReceiver)` (보냄 = viewer→host key, label `b"v2h"`; 받음 = host→viewer key, label `b"h2v"`), `SessionKeys::host_side(&self) -> (SealedSender, SealedReceiver)` (반대)
  - `use chacha20poly1305::aead::Aead;` (최상위 재수출 없음), `Key::from([u8; 32])`, `Nonce::from([u8; 12])`.

- [ ] **Step 1: 실패하는 테스트 작성**

```rust
fn pair() -> ((SealedSender, SealedReceiver), (SealedSender, SealedReceiver)) {
    let k = SessionKeys { viewer_to_host: [1; 32], host_to_viewer: [2; 32], hello_binding: [3; 32] };
    (k.viewer_side(), k.host_side())
}
#[test]
fn roundtrip_both_directions() {
    let ((mut vs, mut vr), (mut hs, mut hr)) = pair();
    assert_eq!(hr.open(&vs.seal(b"offer")).unwrap(), b"offer");
    assert_eq!(vr.open(&hs.seal(b"answer")).unwrap(), b"answer");
}
#[test]
fn replay_rejected() {
    let ((mut vs, _), (_, mut hr)) = pair();
    let m = vs.seal(b"a");
    hr.open(&m).unwrap();
    assert!(matches!(hr.open(&m), Err(AuthError::BadSeal)));
}
#[test]
fn reorder_rejected() {
    let ((mut vs, _), (_, mut hr)) = pair();
    let (m0, m1) = (vs.seal(b"0"), vs.seal(b"1"));
    assert!(hr.open(&m1).is_err());
    assert_eq!(hr.open(&m0).unwrap(), b"0"); // 실패가 상태를 바꾸지 않음
}
#[test]
fn tamper_rejected() {
    let ((mut vs, _), (_, mut hr)) = pair();
    let mut m = vs.seal(b"offer");
    let last = m.len() - 1;
    m[last] ^= 1;
    assert!(hr.open(&m).is_err());
    assert!(hr.open(&[0u8; 5]).is_err()); // 너무 짧음
}
#[test]
fn wrong_direction_rejected() {
    let ((mut vs, _), (_, _)) = pair();
    let ((_, mut vr), _) = pair();
    assert!(vr.open(&vs.seal(b"x")).is_err()); // viewer 가 자기 방향 메시지를 받음
}
```

- [ ] **Step 2: 실패 확인** — `cargo test -p auth sealed`. 기대: 컴파일 실패.
- [ ] **Step 3: 구현** — `sealed.rs`.
- [ ] **Step 4: 통과 확인** — `cargo test -p auth`. 기대: 모두 통과.
- [ ] **Step 5: commit** — "feat: PAKE key 로 signaling 메시지 봉인".

---

### Task 5: `signaling/` Worker (ID 발급, host 대기, viewer 연결, 중계, 요청 제한)

**Files:**
- Create: `signaling/{package.json,tsconfig.json,wrangler.jsonc,vitest.config.ts,worker-configuration.d.ts}` (spike `spikes/signaling` 의 버전·설정을 그대로 가져오고 이름만 `simple-remote-signaling`)
- Create: `signaling/src/{index.ts,host-room.ts,rate-limit.ts,hex.ts,ed25519.ts}`, `signaling/test/{rate-limit.test.ts,register.test.ts,relay.test.ts}`
- Modify: `.github/workflows/ci.yml` (signaling job 은 Task 1 에서 이미 조건부로 있음, 변경 없음을 확인)

**Interfaces (wire protocol, Rust `protocol::signaling` 과 같음):**
- 경로
  - `GET /v1/host?key=<hex32>`: 새 등록. Worker 가 9자리 ID(100000000~999999999, `crypto.getRandomValues` + 거부 표본)를 골라 `HOST_ROOM.idFromName(id)` 로 보낸다. DO 가 409 면 다른 ID 로 최대 5번 다시 (매번 `new Request(request)`).
  - `GET /v1/host/<id>?key=<hex32>`: 기존 ID 로 대기. 저장된 key 와 다르면 403.
  - `GET /v1/viewer/<id>`: viewer 연결. host 가 없으면 404, 이미 viewer 가 있으면 409, 요청 제한이면 429.
  - 그 밖 404, Upgrade 헤더 없으면 426, `CF-Connecting-IP` 없으면 400.
- host 쪽 흐름: 연결 직후 서버 → `{"t":"challenge","nonce":<hex32>,"id":<9자리>}`. host → `{"t":"auth","sig":<hex64>}` (서명 대상 byte: ASCII `"simple-remote signaling auth v1"` || nonce 32 byte || id ASCII 9 byte). 확인 성공 시 ID 를 key 에 묶어 저장하고 `{"t":"registered","id":...}`. 실패 시 close 4003. 인증 전 다른 메시지는 close 4003. 같은 ID 에 새 host 연결이 인증되면 이전 host 연결은 close 4010 으로 닫는다 (재시작한 host 가 이어받음). 인증 전 host 연결은 viewer 연결 판단에서 없는 것으로 친다.
- viewer 연결 시 host 에 `{"t":"viewer_joined"}`, viewer 에 `{"t":"joined"}`. 이후 `relay` 는 반대쪽으로 그대로 전달. host 의 `{"t":"kick"}` → viewer close 4001. viewer 끊김 → host 에 `{"t":"viewer_left"}`. host 끊김 → viewer 에 `{"t":"host_left"}` 후 close 4002.
- 메시지 크기 상한 65,536 byte (설계 제안값). 넘으면 close 4013. JSON 이 아니거나 모르는 `t` 는 close 4000.
- 요청 제한 (설계 제안값, fixed window): viewer 연결 IP 당 10분에 30회, host ID 당 10분에 60회. host 연결 IP 당 10분에 20회. 카운터는 `RATE_LIMIT` DO(`idFromName("ip:"+ip)` / `"id:"+id`)의 `ctx.storage.kv`.
- `rate-limit.ts`: `export function allow(state: Window | undefined, now: number, limit: number, windowMs: number): { ok: boolean; next: Window }` (순수 함수, `Window = { start: number; count: number }`). DO 는 `Date.now()` 를 넘긴다.
- `ed25519.ts`: `export async function verifyEd25519(pubHex: string, sigHex: string, msg: Uint8Array): Promise<boolean>`: 길이(32/64 byte)나 hex 가 틀리거나 `importKey`/`verify` 가 던지면 `false` (fail closed). `crypto.subtle.importKey("raw", pub, { name: "Ed25519" }, false, ["verify"])`.
- 저장: `HostRoom` DO 의 `ctx.storage.kv` 에 `owner` (hex key). WebSocket 은 hibernation API (`ctx.acceptWebSocket(ws, ["host"])` / `["viewer"]`), host 인증 상태는 `serializeAttachment({ authed: boolean, nonce: string })`.

- [ ] **Step 1: 실패하는 테스트 작성**
  - `test/rate-limit.test.ts`: `allow` 가 한도까지 `ok`, 한도+1 에서 거부, 창이 지나면 다시 `ok` (시각을 인자로 넣음).
  - `test/register.test.ts` (도우미: `crypto.subtle.generateKey({ name: "Ed25519" }, true, ["sign","verify"])`, `exportKey("raw")`, 요청에 `CF-Connecting-IP: 203.0.113.1` 헤더):
    - `assigns_nine_digit_id_and_binds_key`: challenge 에 서명 → `registered.id` 가 `/^[1-9]\d{8}$/`.
    - `same_key_keeps_id`: 같은 key 로 `/v1/host/<id>` 재연결 → 같은 `id`.
    - `other_key_is_forbidden`: 다른 key 로 `/v1/host/<id>` → status 403.
    - `bad_signature_closes_4003`: 다른 key 로 서명 → close code 4003.
    - `rejects_short_signature`: sig 63 byte → close 4003 (예외 없이).
    - `rejects_missing_client_ip`: 헤더 없이 → 400.
    - `new_host_connection_replaces_old`: 같은 key 로 두 번째 host 연결 인증 → 첫 연결이 close 4010, 이후 viewer 연결 시 두 번째 host 만 `viewer_joined` 를 받음.
    - `unauthenticated_host_is_not_waiting`: 인증하지 않은 host 연결만 있는 ID 로 viewer → 404.
  - `test/relay.test.ts`:
    - `viewer_to_offline_host_404`
    - `relays_both_ways`: host 인증 → viewer 연결 → host 가 `viewer_joined`, viewer 가 `joined` → 양방향 relay data 가 그대로 도착.
    - `second_viewer_409`
    - `kick_closes_viewer_4001`, `host_close_sends_host_left`, `viewer_close_sends_viewer_left`
    - `oversize_message_closes_4013`
    - `viewer_rate_limited_429`: 같은 IP 로 31번째 viewer 연결 → 429.
- [ ] **Step 2: 실패 확인** — `cd signaling && npm install && npm test`. 기대: 모듈이 없어 실패. (`npm install` 은 npm 11 필요. `npm -v` 가 10 이면 `npm install -g npm@11`, 결과 문서 T8)
- [ ] **Step 3: 구현** — `index.ts`(경로·IP 확인·ID 선택 재시도), `host-room.ts`(`HostRoom` DO), `rate-limit.ts`, `hex.ts`, `ed25519.ts`. `wrangler.jsonc`: DO binding `HOST_ROOM`(`HostRoom`), `RATE_LIMIT`(`RateLimit`), migration `new_sqlite_classes: ["HostRoom","RateLimit"]`, `compatibility_date: "2026-09-01"`. `npm run types` 로 `worker-configuration.d.ts` 재생성 (`wrangler types --include-runtime=false`).
- [ ] **Step 4: 통과 확인** — `npm run typecheck && npm test`. 기대: 모두 통과. WebSocket 테스트 파일이 여러 개라 `package.json` 의 test script 는 `vitest run --max-workers=1 --no-isolate` (Cloudflare 문서: WebSocket 을 쓰는 DO 는 파일별 저장소 격리를 지원하지 않음).
- [ ] **Step 5: commit** — `signaling/package-lock.json` 포함, 파일마다 add. "feat: signaling Worker 로 ID 발급과 메시지 중계".

---

### Task 6: `crates/transport` 의 `Peer` (str0m + UDP)

**Files:**
- Create: `crates/transport/Cargo.toml`, `crates/transport/src/{lib.rs,error.rs,net.rs,peer.rs}`, `crates/transport/tests/peer.rs`

**Interfaces:**
- Produces:
  - `transport::local_ips() -> Vec<IpAddr>`: UDP socket 을 `1.1.1.1:53`, `[2606:4700:4700::1111]:53` 에 `connect`(보내지 않음)해 얻은 local 주소. 실패한 계열은 뺀다. loopback·unspecified 는 넣지 않는다.
  - `transport::Peer`: `new(bind_ips: &[IpAddr]) -> Result<Peer, TransportError>` (IP 마다 UDP socket 하나 `ip:0`, nonblocking, `Candidate::host(addr, "udp")`)
  - `Peer::create_offer(&mut self) -> Result<(String, PendingOffer), TransportError>`: `add_channel_with_config(ChannelConfig { label: "control".into(), ..Default::default() })` (신뢰·순서) 후 SDP 문자열. `PendingOffer` 는 `str0m::change::SdpPendingOffer` 를 감싼 newtype.
  - `Peer::accept_offer(&mut self, sdp: &str) -> Result<String, TransportError>` → answer SDP
  - `Peer::accept_answer(&mut self, pending: PendingOffer, sdp: &str) -> Result<(), TransportError>`
  - `Peer::poll(&mut self, max_wait: Duration) -> Result<Vec<PeerEvent>, TransportError>`: `max_wait` 가 지나거나 event 가 하나라도 생기면 돌아온다. 내부: `poll_output` 을 `Timeout` 까지 돌리며 `Transmit` 은 `t.source` 와 같은 주소의 socket 으로 보내고, 모든 socket 에서 nonblocking `recv_from`(받은 socket 주소를 `Receive.destination`), `Input::Timeout`, 1ms sleep 반복.
  - `enum PeerEvent { Connected, ChannelOpen, Data(Vec<u8>), Closed }` (`Event::Connected`, `Event::ChannelOpen` 의 label `"control"`, `Event::ChannelData`, `Event::Closed`/`ChannelClose`)
  - `Peer::send(&mut self, data: &[u8]) -> Result<(), TransportError>`: control channel `write(true, data)`. `Ok(false)` 면 `TransportError::Backpressure`.
  - `Peer::local_fingerprint(&mut self) -> Vec<u8>` (`direct_api().local_dtls_fingerprint().bytes`), `Peer::remote_fingerprint(&mut self) -> Option<Vec<u8>>` (실제 받은 인증서 지문)
  - `Peer::close(&mut self)` (`Rtc::close`, 실패는 무시)
  - `TransportError { Io(std::io::Error), Rtc(String), Backpressure, NoChannel }`
- 사실 (결과 문서 T4 와 str0m 0.23.1 소스): DTLS 지문 확인은 기본으로 켜져 있고, 불일치면 `poll_output` 이 `Err(RtcError::RemoteSdp("remote fingerprint no match"))` 를 내고 `Rtc` 가 죽는다. `Event::Connected` 는 지문 확인 뒤에만 온다. 이 계획은 지문 확인을 끄지 않는다.

- [ ] **Step 1: 실패하는 테스트 작성** — `tests/peer.rs`:

```rust
use std::net::{IpAddr, Ipv4Addr};
use std::time::{Duration, Instant};
use transport::{Peer, PeerEvent};

const LO: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

fn pump_until(a: &mut Peer, b: &mut Peer, secs: u64, mut done: impl FnMut(&[PeerEvent], &[PeerEvent]) -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(secs);
    let (mut ea, mut eb) = (vec![], vec![]);
    while Instant::now() < end {
        match a.poll(Duration::from_millis(5)) { Ok(v) => ea.extend(v), Err(_) => return false }
        match b.poll(Duration::from_millis(5)) { Ok(v) => eb.extend(v), Err(_) => return false }
        if done(&ea, &eb) { return true; }
    }
    false
}

#[test]
fn loopback_roundtrip() {
    let (mut v, mut h) = (Peer::new(&[LO]).unwrap(), Peer::new(&[LO]).unwrap());
    let (offer, pending) = v.create_offer().unwrap();
    let answer = h.accept_offer(&offer).unwrap();
    v.accept_answer(pending, &answer).unwrap();
    assert!(pump_until(&mut v, &mut h, 10, |a, b| a.contains(&PeerEvent::ChannelOpen) && b.contains(&PeerEvent::ChannelOpen)));
    assert_eq!(v.remote_fingerprint().unwrap(), h.local_fingerprint());
    v.send(b"ping").unwrap();
    assert!(pump_until(&mut v, &mut h, 5, |_, b| b.contains(&PeerEvent::Data(b"ping".to_vec()))));
}

#[test]
fn fingerprint_mismatch_never_opens() {
    let (mut v, mut h, mut other) = (Peer::new(&[LO]).unwrap(), Peer::new(&[LO]).unwrap(), Peer::new(&[LO]).unwrap());
    let (offer, pending) = v.create_offer().unwrap();
    let answer = h.accept_offer(&offer).unwrap();
    let (other_offer, _) = other.create_offer().unwrap();
    let fake = replace_fingerprint(&answer, &other_offer); // answer 의 a=fingerprint 줄을 other 의 것으로
    v.accept_answer(pending, &fake).unwrap();
    assert!(!pump_until(&mut v, &mut h, 5, |a, _| a.contains(&PeerEvent::ChannelOpen)));
}

fn replace_fingerprint(sdp: &str, from: &str) -> String {
    let fp = from.lines().find(|l| l.starts_with("a=fingerprint:")).unwrap();
    sdp.lines().map(|l| if l.starts_with("a=fingerprint:") { fp } else { l }).collect::<Vec<_>>().join("\r\n") + "\r\n"
}

#[test]
fn local_ips_excludes_loopback() {
    assert!(transport::local_ips().iter().all(|ip| !ip.is_loopback() && !ip.is_unspecified()));
}
```

- [ ] **Step 2: 실패 확인** — `cargo test -p transport --test peer`. 기대: 컴파일 실패.
- [ ] **Step 3: 구현** — `peer.rs`, `net.rs`, `error.rs`. 루프 형태는 `spikes/ipv6-pair/src/main.rs` 와 같고 socket 이 여러 개인 점만 다르다.
- [ ] **Step 4: 통과 확인** — `cargo test -p transport --test peer`. 기대: 3 passed. `fingerprint_mismatch_never_opens` 가 실패하면 추측으로 고치지 말고 `poll` 이 돌려준 오류 문자열을 찍어 str0m 이 지문 불일치를 냈는지 확인한다.
- [ ] **Step 5: commit** — "feat: str0m 과 UDP socket 을 묶은 Peer".

---

### Task 7: `crates/transport` 의 signaling `Link` (WebSocket 과 메모리 가짜)

**Files:**
- Create: `crates/transport/src/{link.rs,fake_signal.rs}`, `crates/transport/tests/ws_link.rs`
- Modify: `crates/transport/Cargo.toml` (`[features] test-support = []`, 의존성 `tungstenite`, `rustls`, `serde`, `serde_json`, `protocol`)

**Interfaces:**
- Produces:
  - `trait Link<In, Out> { fn send(&mut self, msg: &Out) -> Result<(), LinkError>; fn recv(&mut self, timeout: Duration) -> Result<Option<In>, LinkError>; }` (`None` = 시간 안에 메시지 없음)
  - `enum LinkError { Closed { code: Option<u16> }, Http { status: u16 }, InsecureUrl, Io(std::io::Error), Protocol(String) }`
  - `WsLink<In, Out>`: `connect(url: &str) -> Result<WsLink<In, Out>, LinkError>`. `ws://` 는 host 가 `127.0.0.1`, `::1`, `localhost` 일 때만 허용하고 그 밖은 `InsecureUrl`. 첫 연결 전에 `rustls::crypto::ring::default_provider().install_default()` 를 한 번 부른다 (결과 무시, 없으면 `wss://` 첫 연결이 panic). 업그레이드 전 HTTP 오류 응답은 `Http { status }` (tungstenite 0.30 `Error::Http` variant 를 `tungstenite-0.30.0/src/error.rs` 에서 확인하고 매핑). 읽기 시간 제한은 `MaybeTlsStream::Plain(s) => s.set_read_timeout`, `MaybeTlsStream::Rustls(s) => s.get_ref().set_read_timeout`, 나머지 `_` 는 `Protocol`. `Error::Io` 의 kind 가 `WouldBlock` 또는 `TimedOut` 이면 `Ok(None)` (Windows 는 `TimedOut`). close frame 은 `Closed { code }`. JSON 은 `Message::Text`.
  - `fake_signal` (`#[cfg(any(test, feature = "test-support"))]`): `FakeHub::new() -> (FakeHub, FakeHostLink)`, `FakeHub::viewer(&self) -> FakeViewerLink` (만들면 host 에 `ViewerJoined`, viewer 에 `Joined`), `FakeHub::host_relays(&self) -> Vec<Vec<u8>>` (host 가 보낸 relay data 를 hex 해독한 기록). `FakeHostLink: Link<ServerToHost, HostToServer>`, `FakeViewerLink: Link<ServerToViewer, ViewerToServer>`. `Kick` → viewer 는 이미 도착한 메시지를 다 읽은 뒤의 `recv` 에서 `Err(Closed { code: Some(4001) })` (실제 서버처럼 순서 유지). `FakeHub: Clone`, `FakeHostLink: Send`, `FakeViewerLink: Send`. viewer drop → host 에 `ViewerLeft`. host drop → viewer 에 `HostLeft`. 동기화는 `std::sync::mpsc` + `Mutex`.

- [ ] **Step 1: 실패하는 테스트 작성** — `tests/ws_link.rs` (테스트 안에서 `std::net::TcpListener` + `tungstenite::accept` 로 작은 서버 thread):

```rust
#[test]
fn ws_roundtrip_and_timeout() {
    let (url, server) = echo_server(); // 받은 text 를 그대로 돌려주는 서버, url = "ws://127.0.0.1:<port>"
    let mut l: WsLink<ServerToViewer, ViewerToServer> = WsLink::connect(&url).unwrap();
    assert!(l.recv(Duration::from_millis(100)).unwrap().is_none());
    l.send(&ViewerToServer::Relay { data: "abcd".into() }).unwrap();
    // echo 서버가 {"t":"relay","data":"abcd"} 를 그대로 보내므로 ServerToViewer::Relay 로 읽힌다
    assert_eq!(l.recv(Duration::from_secs(2)).unwrap(), Some(ServerToViewer::Relay { data: "abcd".into() }));
    drop(l); server.join().unwrap();
}
#[test]
fn http_error_before_upgrade() {
    let url = http_404_server(); // "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n" 를 쓰고 닫는 서버
    assert!(matches!(WsLink::<ServerToViewer, ViewerToServer>::connect(&url), Err(LinkError::Http { status: 404 })));
}
#[test]
fn plain_ws_only_on_loopback() {
    assert!(matches!(WsLink::<ServerToViewer, ViewerToServer>::connect("ws://example.com/v1/viewer/1"), Err(LinkError::InsecureUrl)));
}
```

  `fake_signal.rs` tests: `viewer_join_and_relay` (hub.viewer() → host 가 `ViewerJoined`, 서로 relay 가 도착, `host_relays()` 에 기록), `kick_closes_viewer`, `drops_notify_other_side`.
- [ ] **Step 2: 실패 확인** — `cargo test -p transport --all-features`. 기대: 컴파일 실패.
- [ ] **Step 3: 구현** — `link.rs`, `fake_signal.rs`.
- [ ] **Step 4: 통과 확인** — `cargo test -p transport --all-features`. 기대: 모두 통과.
- [ ] **Step 5: commit** — "feat: signaling Link 와 메모리 가짜 구현".

---

### Task 8: host 시도 제한 (`apps/host-agent` lib)

**Files:**
- Create: `apps/host-agent/Cargo.toml` (`[lib]` + `[[bin]]`), `apps/host-agent/src/{lib.rs,attempts.rs}`, 빈 `main.rs` (`fn main() {}`, Task 10 에서 채움)

**Interfaces:**
- Produces:
  - `host_agent::AttemptLimiter::new() -> AttemptLimiter`
  - `check(&self, now: Instant) -> Result<(), Duration>`: 대기 중이면 `Err(남은 시간)`
  - `record_failure(&mut self, now: Instant) -> FailureOutcome`, `struct FailureOutcome { pub rotate_code: bool }`
  - `record_success(&mut self)`: 연속 실패 수와 대기를 초기화
- 규칙 (spec 5.5): 연속 실패 n 번째 뒤 대기 = `min(2^(n-1) 초, 86_400 초)` (상한은 overflow 방지용 설계 제안값). 코드별 실패 수가 5가 되면 `rotate_code: true` 를 돌려주고 코드별 실패 수만 0 으로. 연속 실패 수(대기 시간)는 코드 교체로 초기화하지 않는다.

- [ ] **Step 1: 실패하는 테스트 작성**

```rust
#[test]
fn backoff_doubles_from_one_second() {
    let t0 = Instant::now();
    let mut l = AttemptLimiter::new();
    assert!(l.check(t0).is_ok());
    l.record_failure(t0);
    assert_eq!(l.check(t0), Err(Duration::from_secs(1)));
    assert!(l.check(t0 + Duration::from_secs(1)).is_ok());
    let t1 = t0 + Duration::from_secs(1);
    l.record_failure(t1);
    assert_eq!(l.check(t1), Err(Duration::from_secs(2)));
}
#[test]
fn fifth_failure_rotates_code_but_keeps_backoff() {
    let mut t = Instant::now();
    let mut l = AttemptLimiter::new();
    for i in 1..=5 {
        let out = l.record_failure(t);
        assert_eq!(out.rotate_code, i == 5);
        t += Duration::from_secs(1 << (i - 1));
    }
    let out = l.record_failure(t);            // 새 코드의 첫 실패
    assert!(!out.rotate_code);
    assert_eq!(l.check(t), Err(Duration::from_secs(32))); // 6번째 연속 실패 → 2^5
}
#[test]
fn success_resets() {
    let t = Instant::now();
    let mut l = AttemptLimiter::new();
    l.record_failure(t);
    l.record_success();
    assert!(l.check(t).is_ok());
}
#[test]
fn backoff_is_capped() {
    let mut t = Instant::now();
    let mut l = AttemptLimiter::new();
    for _ in 0..40 { l.record_failure(t); t += Duration::from_secs(86_400); }
    l.record_failure(t);
    assert_eq!(l.check(t), Err(Duration::from_secs(86_400)));
}
```

- [ ] **Step 2: 실패 확인** — `cargo test -p host-agent attempts`. 기대: 컴파일 실패.
- [ ] **Step 3: 구현** — `attempts.rs`.
- [ ] **Step 4: 통과 확인** — `cargo test -p host-agent`. 기대: 4 passed.
- [ ] **Step 5: commit** — "feat: host 코드 추측 시도 제한".

---

### Task 9: 첫 연결 흐름 (`viewer-core::connect`, `host_agent::serve_next`)

**Files:**
- Create: `crates/viewer-core/Cargo.toml`, `crates/viewer-core/src/{lib.rs,connect.rs}`
- Create: `apps/host-agent/src/session.rs`, `apps/host-agent/tests/first_connection.rs`
- Modify: `apps/host-agent/src/lib.rs`, `apps/host-agent/Cargo.toml` (dev-dependency `viewer-core`, `transport` feature `test-support`)

**Interfaces:**
- Consumes: Task 1~8 의 모든 Produces.
- Produces (viewer):
  - `viewer_core::connect<L: Link<ServerToViewer, ViewerToServer>>(link: L, host_id: &str, code: &OneTimeCode, keys: &DeviceKeys, bind_ips: &[IpAddr], t: &Timeouts) -> Result<ViewerSession, ConnectError>`
  - `ViewerSession { pub host: PeerIdentity, .. }`, `ViewerSession::ping(&mut self, timeout: Duration) -> Result<Duration, ConnectError>` (RTT)
  - `enum ConnectError { WrongCode, RetryAfter(Duration), HostNotWaiting, Busy, RateLimited, HostLeft, Rejected(RejectReason), VersionMismatch { ours: u32, theirs: u32, older: Side }, Timeout(Stage), Security(&'static str), Link(LinkError), Transport(TransportError) }`, `enum Side { Viewer, Host }`, `Stage` 는 `protocol::Stage`. `LinkError::Http { status }` 는 404→`HostNotWaiting`, 409→`Busy`, 429→`RateLimited` 으로 바꾼다 (Task 10 의 viewer 가 이 매핑을 씀; `connect` 는 이미 연결된 link 를 받으므로 이 변환 함수 `ConnectError::from_join_error(LinkError) -> ConnectError` 를 따로 둔다).
- Produces (host):
  - `host_agent::HostConfig { pub host_id: String, pub keys: DeviceKeys, pub bind_ips: Vec<IpAddr>, pub timeouts: Timeouts, pub protocol_version: u32 }` (`protocol_version` 은 기본 `PROTOCOL_VERSION`, 버전 불일치 테스트에서만 바꿈)
  - `host_agent::HostState { pub code: OneTimeCode, pub limiter: AttemptLimiter }`, `HostState::new() -> HostState`
  - `host_agent::ApprovalRequest { pub viewer_name: String, pub viewer_key_fingerprint: String }`
  - `host_agent::serve_next<L: Link<ServerToHost, HostToServer>>(link: &mut L, cfg: &HostConfig, state: &mut HostState, approve: &mut dyn FnMut(&ApprovalRequest, Duration) -> bool) -> Result<ServeOutcome, HostError>`: `ViewerJoined` 를 기다려 viewer 하나를 끝까지 처리하고 돌아온다. `approve` 의 둘째 인자는 허락 제한 시간 (30초).
  - `enum ServeOutcome { Session(HostSession), Failed(AttemptFailure), ViewerLeft }`, `enum AttemptFailure { WaitRequired, NoConfirm, WrongCode, Malformed, Timeout(Stage), Denied, VersionMismatch, BadIdentity, Transport }` (`NoConfirm` = Reply 를 보낸 뒤 viewer 가 확인 MAC 없이 떠남. 틀린 코드의 viewer 는 host MAC 으로 먼저 알아채고 떠나므로 대부분의 틀린 코드는 이것으로 끝난다), `Stage` 는 `protocol::Stage`
  - `HostSession::serve_pings(&mut self, for_at_most: Duration) -> Result<(), HostError>` (Ping 에 Pong, `Closed` 나 시간이 되면 끝)
  - `HostError { Link(LinkError), Transport(TransportError) }` (signaling 자체가 끊긴 경우만. viewer 쪽 문제는 `ServeOutcome::Failed`)
- 메시지 순서 (정확히 이대로, 각 단계 대기는 `t.signaling_step`):
  1. viewer: `Joined` 대기 → `ViewerPake::start` → relay `PakeMsg::Start { pa }`.
  2. host: `ViewerJoined` 뒤 `state.limiter.check(now)` 가 `Err(d)` 면 relay `RetryAfter { ms }` + `Kick` → `Failed(WaitRequired)` (PAKE 하지 않음). 아니면 `Start` 를 받아 `HostPake::respond` → **relay 하기 전에 `limiter.record_failure(now)` 로 실패를 먼저 기록** (`rotate_code` 면 `state.code = OneTimeCode::generate()`. 이번 시도는 이미 계산한 옛 코드로 계속) → relay `Reply { pb, mac_b }`. 먼저 기록하는 이유: 틀린 코드를 쓴 viewer 는 `mac_b` 로 실패를 알고 아무것도 보내지 않고 떠날 수 있어서, 확인 MAC 이 올 때 기록하면 공격자가 대기 시간 없이 추측할 수 있다.
  3. viewer: `finish` 가 `WrongCode` 면 `ConnectError::WrongCode` (주소를 보내지 않고 link 를 닫는다). 성공하면 `Peer::new(bind_ips)`, `create_offer`, `viewer_side()` 로 offer 봉인 → relay `Confirm { mac_a, sealed_offer }`.
  4. host: `ViewerLeft` 나 시간 초과면 `Failed(NoConfirm)` (실패는 2단계에서 이미 기록). `confirm(mac_a)` 실패 → relay `Rejected` + `Kick` → `Failed(WrongCode)`. 봉인 열기 실패·해독 불가 메시지 → `Rejected` + `Kick` → `Failed(Malformed)`. 확인 MAC 이 맞으면 `limiter.record_success()` (봉인 열기가 그 뒤에 실패해도 코드는 맞았으므로 초기화는 유지), `accept_offer` → relay `Answer { sealed_answer }`.
  5. viewer: `accept_answer`. 양쪽이 `ChannelOpen` 까지 `poll` (`t.connect`).
  6. 양쪽이 즉시 `Control::Hello` 를 보낸다 (`make_hello(role, &keys.hello_binding, &peer.local_fingerprint(), &peer.remote_fingerprint()?)`). 상대 Hello 는 `verify_hello(.., sender_fp = 상대 지문(remote), receiver_fp = 자기 지문(local))`.
  7. host: viewer Hello 확인 실패 → `Decision::Rejected(BadIdentity)`. 버전 다름 → `Rejected(VersionMismatch { host_version })`. 확인되면 `approve(&req, t.approval)`: false → `Rejected(Denied)`. true → `Decision::Accepted`, `state.code = OneTimeCode::generate()` → `ServeOutcome::Session`.
  8. viewer: host Hello 확인 실패 → `Security("host identity")` 로 끊음. `Decision` 을 받아 `Accepted` 면 `ViewerSession`. `VersionMismatch { host_version }` 이면 `older` = 작은 쪽.
  - host 는 PAKE 가 끝나기 전 `ViewerLeft` 를 받으면 `ServeOutcome::ViewerLeft`. viewer 는 `HostLeft` 나 link 닫힘을 `ConnectError::HostLeft` 로.

- [ ] **Step 1: 실패하는 테스트 작성** — `apps/host-agent/tests/first_connection.rs`:

```rust
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use auth::{key_fingerprint, DeviceKeys, OneTimeCode, ViewerPake};
use host_agent::*;
use protocol::{control::RejectReason, pake_msg::{self, PakeMsg}, signaling::*, Timeouts, PROTOCOL_VERSION};
use transport::{fake_signal::FakeHub, Link};
use viewer_core::{connect, ConnectError, Side};

const LO: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
const HOST_ID: &str = "123456789";

fn cfg(version: u32) -> HostConfig {
    HostConfig { host_id: HOST_ID.into(), keys: DeviceKeys::generate("엄마 PC").unwrap(),
                 bind_ips: vec![LO], timeouts: Timeouts::default(), protocol_version: version }
}
type Seen = Arc<Mutex<Vec<ApprovalRequest>>>;
fn spawn_host(mut link: impl Link<ServerToHost, HostToServer> + Send + 'static, cfg: HostConfig,
              mut state: HostState, allow: bool, seen: Seen) -> JoinHandle<(ServeOutcome, HostState, HostConfig)> {
    thread::spawn(move || {
        let mut approve = |r: &ApprovalRequest, _: Duration| { seen.lock().unwrap().push(r.clone()); allow };
        let mut out = serve_next(&mut link, &cfg, &mut state, &mut approve).unwrap();
        if let ServeOutcome::Session(s) = &mut out { s.serve_pings(Duration::from_secs(3)).unwrap(); }
        (out, state, cfg)
    })
}
fn relays_of_host(hub: &FakeHub) -> Vec<PakeMsg> {
    hub.host_relays().iter().map(|b| pake_msg::decode(b).unwrap()).collect()
}

#[test]
fn first_connection_succeeds_and_rotates_code() {
    let (hub, host_link) = FakeHub::new();
    let state = HostState::new();
    let code = state.code.clone();
    let seen: Seen = Default::default();
    let h = spawn_host(host_link, cfg(PROTOCOL_VERSION), state, true, seen.clone());
    let vk = DeviceKeys::generate("노트북").unwrap();
    let mut s = connect(hub.viewer(), HOST_ID, &code, &vk, &[LO], &Timeouts::default()).unwrap();
    assert!(s.ping(Duration::from_secs(2)).unwrap() < Duration::from_secs(1));
    let (out, state, cfg) = h.join().unwrap();
    assert!(matches!(out, ServeOutcome::Session(_)));
    assert_ne!(state.code.as_str(), code.as_str());
    assert_eq!(s.host.device_key, cfg.keys.device_public());
    let req = seen.lock().unwrap()[0].clone();
    assert_eq!(req.viewer_name, "노트북");
    assert_eq!(req.viewer_key_fingerprint, key_fingerprint(&vk.device_public()));
}

#[test]
fn wrong_code_leaks_no_addresses() {
    let (hub, host_link) = FakeHub::new();
    let h = spawn_host(host_link, cfg(PROTOCOL_VERSION), HostState::new(), true, Default::default());
    let wrong = OneTimeCode::parse("000000").unwrap(); // generate() 가 000000 을 낼 확률은 무시
    let r = connect(hub.viewer(), HOST_ID, &wrong, &DeviceKeys::generate("v").unwrap(), &[LO], &Timeouts::default());
    assert!(matches!(r, Err(ConnectError::WrongCode)));
    let (out, state, _) = h.join().unwrap();
    assert!(matches!(out, ServeOutcome::Failed(AttemptFailure::NoConfirm)));
    assert!(state.limiter.check(Instant::now()).is_err()); // 떠난 viewer 도 실패로 기록됨
    assert!(!relays_of_host(&hub).iter().any(|m| matches!(m, PakeMsg::Answer { .. })));
}

#[test]
fn retry_after_during_backoff() {
    let (hub, mut host_link) = FakeHub::new();
    let (c, mut state) = (cfg(PROTOCOL_VERSION), HostState::new());
    let wrong = OneTimeCode::parse("000000").unwrap();
    let vk = DeviceKeys::generate("v").unwrap();
    let hub2 = hub.clone();
    let v = thread::spawn(move || {
        let first = connect(hub2.viewer(), HOST_ID, &wrong, &vk, &[LO], &Timeouts::default());
        let second = connect(hub2.viewer(), HOST_ID, &wrong, &vk, &[LO], &Timeouts::default());
        (first, second)
    });
    let mut deny = |_: &ApprovalRequest, _: Duration| false;
    serve_next(&mut host_link, &c, &mut state, &mut deny).unwrap();
    let replies_before = relays_of_host(&hub).iter().filter(|m| matches!(m, PakeMsg::Reply { .. })).count();
    let out = serve_next(&mut host_link, &c, &mut state, &mut deny).unwrap();
    assert!(matches!(out, ServeOutcome::Failed(AttemptFailure::WaitRequired)));
    let (first, second) = v.join().unwrap();
    assert!(matches!(first, Err(ConnectError::WrongCode)));
    match second { Err(ConnectError::RetryAfter(d)) => assert!(d > Duration::ZERO && d <= Duration::from_secs(1)), other => panic!("{other:?}") }
    let replies_after = relays_of_host(&hub).iter().filter(|m| matches!(m, PakeMsg::Reply { .. })).count();
    assert_eq!(replies_before, replies_after); // 대기 중에는 PAKE 응답이 없음
}

#[test]
fn approval_denied_keeps_code() {
    let (hub, host_link) = FakeHub::new();
    let state = HostState::new();
    let code = state.code.clone();
    let h = spawn_host(host_link, cfg(PROTOCOL_VERSION), state, false, Default::default());
    let r = connect(hub.viewer(), HOST_ID, &code, &DeviceKeys::generate("v").unwrap(), &[LO], &Timeouts::default());
    assert!(matches!(r, Err(ConnectError::Rejected(RejectReason::Denied))));
    let (out, state, _) = h.join().unwrap();
    assert!(matches!(out, ServeOutcome::Failed(AttemptFailure::Denied)));
    assert_eq!(state.code.as_str(), code.as_str());
}

#[test]
fn version_mismatch_reports_older_side() {
    let (hub, host_link) = FakeHub::new();
    let state = HostState::new();
    let code = state.code.clone();
    let _h = spawn_host(host_link, cfg(PROTOCOL_VERSION + 1), state, true, Default::default());
    let r = connect(hub.viewer(), HOST_ID, &code, &DeviceKeys::generate("v").unwrap(), &[LO], &Timeouts::default());
    assert!(matches!(r, Err(ConnectError::VersionMismatch { older: Side::Viewer, .. })));
}

#[test]
fn host_leaves_mid_handshake() {
    let (hub, mut host_link) = FakeHub::new();
    let h = thread::spawn(move || {
        assert_eq!(host_link.recv(Duration::from_secs(5)).unwrap(), Some(ServerToHost::ViewerJoined));
        assert!(matches!(host_link.recv(Duration::from_secs(5)).unwrap(), Some(ServerToHost::Relay { .. })));
        drop(host_link); // Start 를 받고 사라짐
    });
    let start = Instant::now();
    let r = connect(hub.viewer(), HOST_ID, &OneTimeCode::generate(), &DeviceKeys::generate("v").unwrap(), &[LO], &Timeouts::default());
    h.join().unwrap();
    assert!(matches!(r, Err(ConnectError::HostLeft)));
    assert!(start.elapsed() < Timeouts::default().signaling_step);
}

#[test]
fn garbage_confirm_counts_as_failure() {
    let (hub, host_link) = FakeHub::new();
    let state = HostState::new();
    let code = state.code.clone();
    let h = spawn_host(host_link, cfg(PROTOCOL_VERSION), state, true, Default::default());
    let mut v = hub.viewer();
    assert_eq!(v.recv(Duration::from_secs(5)).unwrap(), Some(ServerToViewer::Joined));
    let (_vp, pa) = ViewerPake::start(&code, HOST_ID).unwrap();
    v.send(&ViewerToServer::Relay { data: hex::encode(pake_msg::encode(&PakeMsg::Start { pa })) }).unwrap();
    assert!(matches!(v.recv(Duration::from_secs(5)).unwrap(), Some(ServerToViewer::Relay { .. }))); // Reply
    let bad = PakeMsg::Confirm { mac_a: vec![0; 32], sealed_offer: vec![1; 10] };
    v.send(&ViewerToServer::Relay { data: hex::encode(pake_msg::encode(&bad)) }).unwrap();
    let (out, state, _) = h.join().unwrap();
    assert!(matches!(out, ServeOutcome::Failed(AttemptFailure::WrongCode)));
    assert!(state.limiter.check(Instant::now()).is_err());
}
```

  이 테스트가 요구하는 것: `FakeHub: Clone`, `ApprovalRequest: Clone`, `OneTimeCode: Clone`, `FakeHostLink: Send`, `HostConfig.keys` 공개 필드. Task 7·8·9 구현이 이를 만족해야 한다. dev-dependency `hex`.
- [ ] **Step 2: 실패 확인** — `cargo test -p host-agent --test first_connection`. 기대: 컴파일 실패.
- [ ] **Step 3: 구현** — `crates/viewer-core/src/connect.rs`, `apps/host-agent/src/session.rs`. 메시지 해독 실패(`pake_msg::decode`, hex, `control::decode`)는 모두 fail closed 로 처리한다.
- [ ] **Step 4: 통과 확인** — `cargo test --workspace --all-features`. 기대: 전부 통과.
- [ ] **Step 5: commit** — "feat: 첫 연결 흐름 (PAKE, 봉인 SDP, 기기 key 교환, 허락)".

---

### Task 10: 등록, 콘솔 실행 파일, 로컬 end-to-end

**Files:**
- Create: `apps/host-agent/src/register.rs`, `apps/viewer/Cargo.toml`, `apps/viewer/src/main.rs`, `tools/e2e-local.sh`
- Modify: `apps/host-agent/src/main.rs`, `apps/host-agent/src/lib.rs`
- Modify: `docs/superpowers/progress/2026-09-23-remote-control-brainstorming.md` (진행 상태)

**Interfaces:**
- Produces:
  - `host_agent::register_host(server: &str, keys: &DeviceKeys, id: Option<&str>) -> Result<(String, WsLink<ServerToHost, HostToServer>), RegisterError>`: `{server}/v1/host?key=<hex>` (또는 `/v1/host/<id>?key=`) 에 연결 → `Challenge { nonce, id }` (기존 ID 로 연결했으면 `id` 가 같은지 확인) → `Auth { sig: hex(keys.sign_raw(b"simple-remote signaling auth v1" || nonce || id)) }` → `Registered { id }` 의 id 가 challenge 의 id 와 같은지 확인. 어긋나면 `RegisterError::Protocol`. `RegisterError { Link(LinkError), Protocol(&'static str) }`.
  - `host-agent` 실행: `host-agent --server <url> [--name <이름>] [--once]`. 표준 출력 한 줄씩 `ID <9자리>`, `CODE <6자리>` (코드가 바뀔 때마다 다시), 허락 질문은 `APPROVE? <viewer 이름> <key 지문> [y/N]` 을 출력하고 stdin 한 줄을 30초 기다린다 (시간 초과·EOF·`y` 외 입력은 거절). `--once` 는 세션 하나가 끝나면 종료 코드 0.
  - `viewer` 실행: `viewer --server <url> --id <9자리> --code <6자리> [--name <이름>]`. 성공 시 `CONNECTED <host 이름> <host key 지문>` 과 `PONG rtt_ms=<n>` 3번 출력 후 종료 코드 0. 실패 시 `ConnectError` 를 사람이 읽을 문장으로(`WrongCode` → "코드가 틀렸습니다", `RetryAfter(d)` → "N초 뒤 다시 시도하세요", `HostNotWaiting` → "상대가 대기 중이 아닙니다", 버전 불일치 → 어느 쪽이 구버전인지) 출력하고 종료 코드 1. 코드는 출력하지 않는다.
  - bind 주소는 `transport::local_ips()`, 비어 있으면 `127.0.0.1`.
- [ ] **Step 1: 실패하는 end-to-end script 작성** — `tools/e2e-local.sh` (bash, `set -euo pipefail`):
  1. `cargo build -p host-agent -p viewer`
  2. `(cd signaling && npx wrangler dev --port 8787 --ip 127.0.0.1 --show-interactive-dev-session=false) &` 후 `curl` 로 `http://127.0.0.1:8787/` 가 응답(404)할 때까지 최대 30초 대기
  3. `coproc HOST { yes y | target/debug/host-agent --server ws://127.0.0.1:8787 --name e2e-host --once; }` 의 출력에서 `ID`, `CODE` 를 읽음
  4. `target/debug/viewer --server ws://127.0.0.1:8787 --id "$ID" --code 000000` → 종료 코드 1 과 "코드가 틀렸습니다" 확인
  5. 2초 대기(한 번 틀린 뒤 대기 시간 1초) 후 3에서 읽은 `CODE` 로 `viewer ... --code "$CODE"` (코드는 5번 틀려야 바뀌므로 그대로 유효) → 종료 코드 0, `PONG` 3줄 확인
  6. 모든 자식 종료, 마지막 줄 `E2E OK`
- [ ] **Step 2: 실패 확인** — `bash tools/e2e-local.sh`. 기대: `host-agent`/`viewer` 실행 파일이 아직 콘솔 흐름이 없어 실패.
- [ ] **Step 3: 구현** — `register.rs`, 두 `main.rs` (인자 파싱은 `std::env::args` 로 직접, 새 의존성 없음).
- [ ] **Step 4: 통과 확인** — `bash tools/e2e-local.sh`. 기대: 마지막 줄 `E2E OK`. 그리고 `cargo test --workspace --all-features`, `cd signaling && npm test` 가 모두 통과.
- [ ] **Step 5: 진행 기록 갱신** — 진행 기록 "현재 상태와 인계" 절과 "다음 할 일"에 이 계획의 완료, e2e 결과, 다음 계획 후보(재접속 허가증 + Noise KK + 기기 key 저장)를 적는다.
- [ ] **Step 6: commit** — "feat: host-agent·viewer 콘솔 실행 파일과 로컬 end-to-end 확인".

---

## spec 덮임 점검

| spec 요구 | task |
|---|---|
| 5.1 서버가 코드·주소·지문·세션 내용을 못 봄, 서버를 믿지 않음 | 4 (봉인), 9 (SDP 는 봉인으로만), 6 (지문 확인) |
| 5.1 모든 연결 `wss://` | 7 (`InsecureUrl`) |
| 5.2 1단계 숫자 6자리 일회용 코드 | 3 |
| 5.2 3단계 PAKE 먼저, key 확인 MAC 뒤 진행 | 3, 9 (순서 1~4) |
| 5.2 4단계 PAKE key 로 SDP AEAD 보호, 지문이 보호된 SDP 안 | 4, 9 |
| 5.2 5단계 DTLS 지문 확인, 장기 공개키·이름 교환과 서명 증명 | 6, 2, 9 (순서 6) |
| 5.2 6단계 허락 30초, 처음 보는 기기 표시 | 9 (`approve` 제한 시간), 10 (콘솔 질문). "이전에 연결한 기기" 표시는 기기 저장이 생기는 다음 계획 |
| 5.2 7단계 사용한 코드 폐기·새 코드 | 9 |
| 4절 접속 ID 9자리, 서버 발급, 공개키에 묶음, 등록마다 서명 | 5, 10 |
| 4절 재접속 key (X25519, Ed25519 서명 묶음) | 2 (교환·확인만, 사용은 다음 계획) |
| 5.5 코드 추측 대기 2배·5회 교체·유지·성공 시 초기화 | 8, 9 |
| 5.5 signaling IP 별·ID 별 요청 제한 | 5 |
| 5.5 주소 후보는 PAKE 성공 뒤 암호화해서만 | 9 (`wrong_code_leaks_no_addresses`) |
| 5.6 실패 안내 범주 (대기 중 아님, 코드 틀림·남은 대기, 거절, 버전) | 9 (`ConnectError`), 10 (문장) |
| 7.4 protocol 버전 교환, 구버전 쪽 표시 | 1, 9, 10 |
| 7.5 Workers + DO 무료, 서버 주소 설정 | 5, 10 (`--server`) |
| 7.6 코드·key 로그 금지 | 3 (`Debug`), 10 |
| 8.1 보안 경로 fail closed | 2, 3, 4, 6, 9 |
| 9.1 단위(메시지, PAKE RFC vector, 대기 시간 가짜 시계), signaling 테스트, 연결 통합 | 1~9 |
| 10.4 "한 코드에 5번 틀리면 교체", "지문 바꿔치기 차단" | 8, 6 |

다음 계획으로 넘긴 spec 요구: 5.3 재접속, 5.4 경로 경주·기록, 5.6 접속 기록, 6~7절, 8.2~8.3.

빈칸 점검: 이 문서에서 "TBD", "나중에", "적절히" 를 검색해 0건.
