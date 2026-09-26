# SP1 재접속 허가증과 저장소 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 한 번 허락받은 viewer 가 끊긴 뒤 1시간 안에는 코드·허락 없이 Noise KK 로 다시 연결하고, 기기 key·접속 ID·허가증·알고 있는 상대가 DPAPI 로 암호화된 파일에 남아 host·viewer 를 다시 시작해도 유지되게 한다. 연결 뼈대의 Kick 경쟁도 viewer 번호로 없앤다.

**Architecture:** signaling 은 viewer 마다 번호를 매기고(host 가 보내는 `relay`/`kick` 은 번호가 맞을 때만 적용), viewer 는 접속 종류(`new`/`reconnect`)를, host 는 모드(`new`/`reconnect`)를 밝힌다. 재접속은 `crates/auth` 의 Noise KK(`snow`) 로 서로의 재접속 key 를 확인하고, 허가증 번호는 Noise 첫 메시지의 암호화된 payload 에만 싣는다(서버가 보지 못함, spec 5.1). 허가증·알고 있는 상대는 `auth` 의 순수 장부(`HostBook`, `ViewerBook`)로 다루고, 저장은 `Protector` trait(Windows DPAPI, 개발용 평문은 feature 로만) 뒤의 `StateFile` 이 원자적으로 쓴다.

**Tech Stack:** 연결 뼈대와 같음 + `snow =0.10.0` (Noise_KK_25519_ChaChaPoly_BLAKE2s), `windows =0.62.2` (DPAPI), `zeroize` `Zeroizing`.

**Spec:** `docs/superpowers/specs/2026-09-24-simple-remote-sp1-design.md` (4, 5.1, 5.3, 5.5 노출 시간, 7.2, 8.1). 근거: `docs/research/2026-09-23-sp1-phase0-spike-results.md` (T3 Noise KK, T9 DPAPI), `spikes/auth/tests/noise_kk.rs`, `spikes/win-probes/src/bin/dpapi_probe.rs`. 이전 계획: `docs/superpowers/plans/2026-09-26-sp1-connection-skeleton.md`.

## 이 계획의 범위

- 포함: signaling viewer 번호·접속 종류·host 모드, Rust 쪽 적용(Kick 경쟁 해소), Noise KK 재접속 key 합의와 봉인, 허가증 장부(발급·세션·대기·1시간 만료·폐기·모두 취소), 알고 있는 상대("이전에 연결한 기기" 표시), 기기 key 직렬화, `StateFile` + `Protector`(DPAPI, 개발용 평문 feature), `crates/platform-win` 과 Windows CI, host·viewer 재접속 흐름, viewer 자동 재시도 간격, 콘솔 실행 파일의 저장·ID 유지·재접속만 모드·가족 끊기·모두 취소, end-to-end 확장.
- 사용자 결정 (저장 범위): 저장소는 폴더를 받는 형태로 만들고, 이번 계획에서는 host 도 사용자 계정으로 도는 콘솔이므로 host·viewer 모두 사용자 계정 DPAPI 로 사용자 폴더(`%LOCALAPPDATA%\SimpleRemote\host`, `%APPDATA%\SimpleRemote\viewer`)에 저장한다. 서비스 계획에서 host 를 SYSTEM 으로 돌리며 폴더만 `ProgramData\SimpleRemote` (SYSTEM 전용 ACL) 로 바꾼다. DPAPI 호출은 그대로다: SYSTEM 으로 실행하면 같은 호출이 SYSTEM 계정 범위가 된다 (spec 4절, `CRYPTPROTECT_LOCAL_MACHINE` 은 쓰지 않음).
- 다음 계획으로 미룸: Windows 서비스·세션 agent·host-ui, 재부팅 후 로그인 화면 대기(서비스 필요), "세션 중" 모드와 ICE restart(8.3), 세션 중 signaling 처리, 접속 기록(5.6), STUN·경로 기록.

## Global Constraints

- 연결 뼈대의 Global Constraints 를 모두 이어받는다 (Rust 1.95, `rand_core` 0.10 세대만, str0m·pakery 고정 버전, fail closed, 코드·key·봉인 전 SDP·허가증 번호 출력 금지, `wss://` 만 등).
- `snow = "=0.10.0"`, 패턴 `Noise_KK_25519_ChaChaPoly_BLAKE2s`. 정적 key 는 `DeviceKeys` 의 X25519 재접속 key (D26). snow 에 넘기는 key 는 정확히 32 byte.
- 허가증: host 가 세션 시작 때 발급, 번호 16 byte 무작위. 상태 세션 중 / 대기(끊긴 시각) / 폐기(장부에서 삭제). 만료 = 끊긴 시각 + 1시간, 재접속해 세션이 다시 시작되면 다음 끊김부터 다시 1시간. 허가증은 번호만으로 쓸 수 없고 viewer 기기 key(재접속 key 로 Noise KK)로 인증해야 한다 (spec 5.3).
- 폐기 조건: 가족이 끊기(콘솔에서는 세션 중 `x` 입력), 끊긴 뒤 1시간, "재접속 허가 모두 취소"(콘솔 `--revoke-all`) (spec 5.3). 프로그램 제거는 설치 계획.
- 재접속은 허락 창 없이 세션을 시작하고 host 에 "재접속됨" 을 알린다 (spec 5.3 4단계).
- 자동 재시도 간격: 1, 2, 5, 10, 30초, 이후 60초마다, 허가증 만료까지 (spec 5.3, 설계 제안값). viewer 가 스스로 끊은 경우 자동 재시도하지 않는다.
- signaling 서버는 코드·허가증·주소 후보를 보지 못한다 (spec 5.1). 허가증 번호는 Noise 암호화 payload 안에만 둔다.
- host 는 host-ui 가 열려 있을 때(콘솔: 기본 모드)만 새 연결을 받고, 유효 허가증만 있을 때는 "재접속만 받기" 로 등록한다 (spec 5.1, 5.5 노출 시간). 서버는 모드에 맞지 않는 viewer 접속을 404 로 거절한다.
- 저장: host·viewer 모두 DPAPI(`CryptProtectData`, `CRYPTPROTECT_UI_FORBIDDEN`, `CRYPTPROTECT_LOCAL_MACHINE` 금지)로 암호화한 파일 하나. 쓰기는 임시 파일에 쓴 뒤 교체(원자적). 파일이 깨졌거나 풀리지 않으면 오류로 멈추고 새로 만들어 덮어쓰지 않는다 (fail closed). 평문 저장은 feature `insecure-dev-store` 를 켠 개발 빌드(Linux e2e)에서만 컴파일된다.
- 접속 ID 는 처음 등록 때 받은 것을 저장해 다시 쓴다 (spec 4절 "host 공개키에 묶는다").

## Review Focus

1. **정확히 1시간 경계의 허가증**: 끊긴 지 정확히 1시간이 된 허가증은 만료로 본다. → Task 4 `expiry_boundary_is_exclusive`.
2. **깨진 저장 파일**: 잘리거나 변조된 파일은 오류가 나고, 새 key 로 조용히 덮어쓰지 않는다 (새 key 면 ID·허가증이 모두 바뀜). → Task 5 `corrupt_file_is_error_and_left_untouched`, Task 8 e2e 의 깨진 파일 단계.
3. **늦은 kick**: 앞 viewer 에게 보낸 `kick` 이 그 사이 들어온 다음 viewer 를 끊지 않는다. → Task 1 `stale_kick_and_relay_are_ignored`, Task 2 `fake_ignores_stale_kick`.
4. **host 재시작**: 같은 데이터 폴더로 다시 시작한 host 는 같은 ID 와 허가증을 유지하고, viewer 가 코드 없이 재접속한다. → Task 8 e2e.
5. **남의 허가증 번호**: 허가증 번호만 알고 다른 기기 key 로 재접속하면 거부되고 host 는 주소(answer)를 보내지 않는다. → Task 7 `copied_permit_id_with_other_keys_is_refused`.

---

## 파일 구조

```
signaling/src/host-room.ts, index.ts        viewer 번호, 접속 종류, host 모드 (Task 1)
signaling/test/numbering.test.ts            (Task 1)
crates/protocol/src/signaling.rs            JoinKind, HostMode, 번호 붙은 메시지 (Task 2)
crates/protocol/src/reconnect_msg.rs        ReconnectMsg (Task 7)
crates/protocol/src/control.rs              Control::Permit, RejectReason::PermitInvalid (Task 7)
crates/transport/src/fake_signal.rs         번호·종류·모드를 Worker 와 같게 (Task 2)
crates/auth/src/reconnect.rs                Noise KK: ViewerReconnect, HostReconnect, NoiseChannel (Task 3)
crates/auth/src/identity.rs                 DeviceKeys 직렬화, PeerIdentity serde (Task 3)
crates/auth/src/permit.rs                   HostBook, ViewerBook, 만료 규칙 (Task 4)
crates/auth/src/store.rs                    Protector, StateFile, DevPlaintext (Task 5)
crates/platform-win/src/{lib,dpapi}.rs      DpapiProtector (Task 6)
.github/workflows/windows-platform.yml      Windows 빌드·DPAPI 테스트 (Task 6)
apps/host-agent/src/session.rs              번호 적용(Task 2), 재접속·허가증 발급(Task 7)
apps/host-agent/src/data.rs                 HostData 저장 형식 (Task 8)
crates/viewer-core/src/connect.rs           번호 무관(viewer 쪽), 허가증 수신(Task 7)
crates/viewer-core/src/reconnect.rs         reconnect(), retry_delays() (Task 7)
crates/viewer-core/src/data.rs              ViewerData 저장 형식 (Task 8)
apps/host-agent/tests/reconnect.rs          (Task 7)
apps/host-agent/src/main.rs, apps/viewer/src/main.rs, tools/e2e-local.sh   (Task 8)
```

---

### Task 1: signaling 의 viewer 번호, 접속 종류, host 모드

**Files:**
- Modify: `signaling/src/host-room.ts`, `signaling/src/index.ts`
- Create: `signaling/test/numbering.test.ts`
- Modify: 기존 `signaling/test/*.test.ts` 와 `test/helpers.ts` (새 wire 에 맞춤)

**Interfaces (wire, Task 2 의 Rust 타입과 같음):**
- viewer 경로: `GET /v1/viewer/<id>?kind=new` 또는 `?kind=reconnect`. `kind` 가 없거나 다른 값이면 400 (IP 확인 뒤, 요청 제한 앞).
- host → 서버, 인증 뒤: `{"t":"mode","mode":"new"}` 또는 `{"t":"mode","mode":"reconnect"}`. 인증 직후 기본 모드는 `reconnect` (새 연결을 받으려면 host 가 `new` 를 밝혀야 함). 다른 값은 close 4000. host attachment 에 `mode` 저장.
- viewer 접속 허용: `kind=new` 는 모드 `new` 일 때만, `kind=reconnect` 는 `new` 또는 `reconnect` 일 때. 아니면 404 (host 가 없을 때와 같음).
- viewer 번호: DO 의 `ctx.storage.kv` 카운터 `next_viewer` (1 부터, 받아들인 viewer 마다 1 증가, 재시작해도 유지). viewer attachment `{ closedByServer, n, kind }`.
- 서버 → host: `{"t":"viewer_joined","n":<number>,"kind":"new"|"reconnect"}`, `{"t":"viewer_left","n":<number>}`, `{"t":"relay","n":<number>,"data":<hex>}`.
- host → 서버: `{"t":"relay","n":<number>,"data":<hex>}` 는 열린 viewer 의 번호가 `n` 일 때만 전달, 아니면 버림. `{"t":"kick","n":<number>}` 도 번호가 맞을 때만 close 4001. `n` 이 없거나 정수가 아니면 close 4000.
- viewer 쪽 메시지(`joined`, `relay`, `host_left`)는 바뀌지 않는다.

- [ ] **Step 1: 실패하는 테스트 작성** — `test/numbering.test.ts` (기존 helpers 사용, host 인증 뒤 모드 설정 도우미 추가):
  - `viewer_numbers_increase`: viewer 연결·종료 두 번 → host 가 받은 `viewer_joined.n` 이 1, 2 이고 `viewer_left.n` 도 같음.
  - `stale_kick_and_relay_are_ignored`: viewer 1 연결 후 종료, viewer 2 연결 → host 가 `{"t":"kick","n":1}` 과 `{"t":"relay","n":1,"data":"aa"}` 를 보냄 → viewer 2 는 열린 채이고 아무것도 받지 않음. 이어서 `relay n=2` 는 viewer 2 에 도착, `kick n=2` 는 viewer 2 를 4001 로 닫음.
  - `mode_gates_join_kind`: 인증만 한 host(기본 `reconnect`) → `kind=new` 404, `kind=reconnect` 101. `mode new` 뒤 → `kind=new` 101.
  - `missing_or_bad_kind_400`, `kick_without_n_closes_host_4000`, `bad_mode_closes_4000`.
  - 기존 테스트는 새 wire 로 고친다 (viewer 경로에 `?kind=new`, host 는 인증 뒤 `mode new`, relay·kick 에 `n`).
- [ ] **Step 2: 실패 확인** — `cd signaling && npm test`. 기대: 새 테스트 실패.
- [ ] **Step 3: 구현** — `index.ts` (kind 검사), `host-room.ts` (모드, 번호, 번호 확인).
- [ ] **Step 4: 통과 확인** — `npm run typecheck && npm test`. 기대: 모두 통과.
- [ ] **Step 5: commit** — "feat: signaling viewer 번호와 접속 종류, host 모드".

---

### Task 2: Rust 쪽 번호·종류·모드 적용 (Kick 경쟁 해소)

**Files:**
- Modify: `crates/protocol/src/signaling.rs`, `crates/transport/src/fake_signal.rs`, `apps/host-agent/src/session.rs`, `apps/host-agent/src/lib.rs`, `apps/host-agent/src/main.rs`, `apps/viewer/src/main.rs`, `apps/host-agent/tests/first_connection.rs`, `crates/transport/tests/*` (새 타입에 맞춤)

**Interfaces:**
- Produces:
  - `protocol::signaling::JoinKind { New, Reconnect }`, `HostMode { New, Reconnect }` (serde `rename_all = "snake_case"`, 값 `"new"`, `"reconnect"`)
  - `ServerToHost { Challenge { nonce, id }, Registered { id }, ViewerJoined { n: u64, kind: JoinKind }, ViewerLeft { n: u64 }, Relay { n: u64, data: String } }`
  - `HostToServer { Auth { sig }, Mode { mode: HostMode }, Relay { n: u64, data: String }, Kick { n: u64 } }`
  - `FakeHub::viewer(&self, kind: JoinKind) -> Result<FakeViewerLink, LinkError>`: Worker 와 같게 모드 불일치·host 없음 → `Http { status: 404 }`, 열린 viewer 있음 → `Http { status: 409 }` (연결 뼈대의 "교체" 동작을 버림). host 가 `Mode` 를 보내기 전 기본 모드는 `Reconnect`. 번호는 1 부터. host 의 `Relay`/`Kick` 은 번호가 맞을 때만 적용.
  - `host_agent::set_mode<L: Link<ServerToHost, HostToServer>>(link: &mut L, mode: HostMode) -> Result<(), LinkError>`
  - `serve_next` 는 `ViewerJoined { n, kind }` 로 현재 번호를 잡고, 그 번호의 `Relay`/`ViewerLeft` 만 처리한다 (다른 번호는 버림). 보내는 `Relay`/`Kick` 에 그 번호를 싣는다. `kind: Reconnect` 는 Task 7 전까지 `Kick { n }` + `ServeOutcome::Failed(AttemptFailure::Malformed)`.
  - R9 예외(떠난 viewer 에게 Kick 하지 않음)는 없앤다: 이제 늦은 Kick 은 번호가 달라 서버가 버린다. 실패 뒤에는 항상 `Kick { n }`.
  - viewer 콘솔은 `.../v1/viewer/<id>?kind=new` 로 연결. host 콘솔은 등록 직후 `set_mode(New)`.

- [ ] **Step 1: 실패하는 테스트 작성**
  - `signaling.rs` tests: `{"t":"viewer_joined","n":3,"kind":"reconnect"}` → `ViewerJoined { n: 3, kind: Reconnect }`; `HostToServer::Kick { n: 2 }` → `{"t":"kick","n":2}`; `HostToServer::Mode { mode: HostMode::New }` → `{"t":"mode","mode":"new"}`; `HostToServer::Relay { n: 1, data: "ab".into() }` → `{"t":"relay","n":1,"data":"ab"}`.
  - `fake_signal.rs` tests: `fake_ignores_stale_kick` (viewer 1 drop, viewer 2 join, host `Kick { n: 1 }` → viewer 2 `recv` 는 `Ok(None)`), `fake_mode_gates_join` (host 가 `Mode` 전 `viewer(New)` → `Err(Http{404})`, `viewer(Reconnect)` → Ok), `fake_second_viewer_is_busy` (409).
  - `first_connection.rs`: `spawn_host` 에서 `serve_next` 전에 `set_mode(&mut link, HostMode::New)`, `hub.viewer()` → `hub.viewer(JoinKind::New).unwrap()`. 다른 단언은 그대로 둔다 (`wrong_code_leaks_no_addresses` 의 host 결과도 `Failed(NoConfirm)`).
- [ ] **Step 2: 실패 확인** — `cargo test --workspace --all-features`. 기대: 컴파일 실패.
- [ ] **Step 3: 구현.**
- [ ] **Step 4: 통과 확인** — `cargo test --workspace --all-features`. 그리고 `bash tools/e2e-local.sh` 가 `E2E OK` (Task 1 Worker 와 맞물림 확인).
- [ ] **Step 5: commit** — "fix: viewer 번호로 늦은 kick 과 relay 가 다음 viewer 에 닿지 않게 함".

---

### Task 3: `crates/auth` Noise KK 재접속과 기기 key 직렬화

**Files:**
- Create: `crates/auth/src/reconnect.rs`
- Modify: `crates/auth/src/identity.rs`, `crates/auth/src/error.rs`, `crates/auth/src/lib.rs`, `crates/auth/Cargo.toml` (`snow = { workspace = true }`), 루트 `Cargo.toml` (`snow = "=0.10.0"`)

**Interfaces:**
- Produces:
  - `AuthError::UnknownPeer` (재접속 상대를 어느 허가증으로도 확인하지 못함), `AuthError::Handshake` (Noise 메시지 처리 실패)
  - `PeerIdentity` 에 `Serialize, Deserialize, Clone, PartialEq, Eq` derive
  - `DeviceKeys::to_secret_bytes(&self) -> zeroize::Zeroizing<Vec<u8>>` = postcard(`{ name: String, signing: [u8; 32], reconnect: [u8; 32] }`), `DeviceKeys::from_secret_bytes(bytes: &[u8]) -> Result<DeviceKeys, AuthError>` (이름 규칙 확인, 해독 실패 `Malformed("device keys")`)
  - `auth::NoiseChannel`: `seal(&mut self, plaintext: &[u8]) -> Vec<u8>`, `open(&mut self, msg: &[u8]) -> Result<Vec<u8>, AuthError>` (snow `TransportState` 를 감쌈, 순서가 어긋나거나 변조되면 `BadSeal`)
  - `auth::ReconnectSession { pub binding: [u8; 32], pub channel: NoiseChannel }` (`binding` = Noise handshake hash 32 byte, Hello 서명의 binding 으로 씀)
  - `auth::ViewerReconnect::start(keys: &DeviceKeys, host_id: &str, host_reconnect_key: &[u8; 32], permit_id: &[u8; 16]) -> Result<(ViewerReconnect, Vec<u8>), AuthError>` → Noise 첫 메시지 (payload = permit_id)
  - `ViewerReconnect::finish(self, msg2: &[u8]) -> Result<ReconnectSession, AuthError>` (실패 `Handshake`)
  - `auth::HostReconnect::respond(keys: &DeviceKeys, host_id: &str, msg1: &[u8], candidates: &[([u8; 16], [u8; 32])]) -> Result<(ReconnectSession, [u8; 16], Vec<u8>), AuthError>` → (세션, 확인된 허가증 번호, 두 번째 메시지). 후보(허가증 번호, viewer 재접속 공개키)마다 새 responder 로 `read_message` 를 시도해 처음 성공하고 payload 가 그 후보의 번호와 같은 것을 고른다. 없으면 `UnknownPeer`. 후보는 최대 64개만 본다 (설계 제안값).
- 정한 값: prologue = `b"simple-remote reconnect v1\0"` || host_id (ASCII). viewer = initiator, host = responder. 두 번째 메시지 payload 는 비어 있음.

- [ ] **Step 1: 실패하는 테스트 작성** — `reconnect.rs` tests (도우미: `DeviceKeys::generate`, 허가증 번호 `[7u8; 16]`):

```rust
#[test]
fn reconnect_roundtrip_and_seal() {
    let (v, h) = (DeviceKeys::generate("v").unwrap(), DeviceKeys::generate("h").unwrap());
    let id = [7u8; 16];
    let (vr, m1) = ViewerReconnect::start(&v, "123456789", &h.reconnect_public(), &id).unwrap();
    let cands = [([1u8; 16], [9u8; 32]), (id, v.reconnect_public())];
    let (mut hs, got, m2) = HostReconnect::respond(&h, "123456789", &m1, &cands).unwrap();
    assert_eq!(got, id);
    let mut vs = vr.finish(&m2).unwrap();
    assert_eq!(vs.binding, hs.binding);
    let c = vs.channel.seal(b"offer");
    assert_eq!(hs.channel.open(&c).unwrap(), b"offer");
    assert!(hs.channel.open(&c).is_err()); // 재전송 거부
    let a = hs.channel.seal(b"answer");
    assert_eq!(vs.channel.open(&a).unwrap(), b"answer");
}
#[test]
fn unknown_viewer_key_is_refused() { /* 후보에 v 의 key 가 없음 → Err(UnknownPeer) */ }
#[test]
fn payload_must_match_candidate_id() { /* 후보 ([8;16], v.reconnect_public()) 이고 viewer 는 id [7;16] → Err(UnknownPeer) */ }
#[test]
fn viewer_with_wrong_host_key_is_refused() { /* viewer 가 다른 host key 로 시작 → host respond Err(UnknownPeer) */ }
#[test]
fn host_id_is_bound() { /* viewer "111111111", host "222222222" → Err(UnknownPeer) */ }
#[test]
fn tampered_second_message_fails() { /* m2 마지막 byte 뒤집기 → finish Err(Handshake) */ }
```

  `identity.rs` tests: `secret_bytes_roundtrip` (복원한 key 의 `device_public`, `reconnect_public`, `name` 이 같고 서명이 원래 공개키로 확인됨), `corrupt_secret_bytes_rejected` (잘린 bytes → `Malformed`).
- [ ] **Step 2: 실패 확인** — `cargo test -p auth`. 기대: 컴파일 실패.
- [ ] **Step 3: 구현** — snow 사용법은 `spikes/auth/tests/noise_kk.rs`. snow 의 local private key 에는 `DeviceKeys` 의 재접속 `StaticSecret` bytes 를 넣는다 (Zeroizing 으로 감싸 넘기고 버림).
- [ ] **Step 4: 통과 확인** — `cargo test -p auth`.
- [ ] **Step 5: commit** — "feat: Noise KK 재접속 key 합의와 기기 key 직렬화".

---

### Task 4: `crates/auth` 허가증 장부

**Files:**
- Create: `crates/auth/src/permit.rs`
- Modify: `crates/auth/src/lib.rs`

**Interfaces:**
- Produces:
  - `auth::PERMIT_TTL: Duration = 1시간`
  - `auth::unix_ms(t: SystemTime) -> u64`
  - `auth::PermitState { Session, Waiting { since_ms: u64 } }` (serde)
  - `auth::HostPermit { pub id: [u8; 16], pub viewer: PeerIdentity, pub state: PermitState }` (serde)
  - `auth::HostBook` (serde, `Default`):
    - `issue(&mut self, viewer: &PeerIdentity) -> [u8; 16]`: 무작위 번호, 상태 `Session`, 알고 있는 상대에 기록(같은 기기 key 는 이름·재접속 key 갱신)
    - `is_known(&self, device_key: &[u8; 32]) -> bool`
    - `reconnect_candidates(&self, now: SystemTime) -> Vec<([u8; 16], [u8; 32])>`: 만료 전 `Waiting` 만 (세션 중인 허가증은 후보가 아님)
    - `begin_reconnect(&mut self, id: &[u8; 16], device_key: &[u8; 32], now: SystemTime) -> Result<(), PermitError>`: `Waiting` 이고 만료 전이고 기기 key 가 같으면 `Session`
    - `end_session(&mut self, id: &[u8; 16], now: SystemTime)`: `Session` → `Waiting { since_ms: now }`
    - `revoke(&mut self, id: &[u8; 16])`, `revoke_all(&mut self)`: 장부에서 삭제 (알고 있는 상대는 남김)
    - `prune(&mut self, now: SystemTime)`: 만료된 `Waiting` 삭제
    - `has_valid(&self, now: SystemTime) -> bool`: `Session` 이나 만료 전 `Waiting` 이 있는가 (재접속만 받기 등록 조건)
  - `auth::PermitError { Unknown, Expired, InSession, WrongDevice }`
  - `auth::ViewerPermit { pub host_id: String, pub permit_id: [u8; 16], pub host: PeerIdentity, pub disconnected_ms: Option<u64> }` (serde)
  - `auth::ViewerBook` (serde, `Default`): `store(&mut self, p: ViewerPermit)` (같은 host_id 는 교체), `get(&self, host_id: &str) -> Option<&ViewerPermit>`, `mark_disconnected(&mut self, host_id: &str, now: SystemTime)`, `remove(&mut self, host_id: &str)`, `expired(&self, host_id: &str, now: SystemTime) -> bool` (viewer 쪽 추정: `disconnected_ms` + 1시간)
- 만료 규칙: `now_ms >= since_ms + 3_600_000` 이면 만료.

- [ ] **Step 1: 실패하는 테스트 작성** — `permit.rs` tests (시각은 `UNIX_EPOCH + Duration::from_secs(1_000_000)` 기준으로 만듦):
  - `issue_then_end_then_reconnect`: issue → candidates 비어 있음(세션 중) → end_session(t) → candidates 에 (id, viewer.reconnect_key) → begin_reconnect(id, viewer.device_key, t+59분) Ok → 다시 `Session`.
  - `expiry_boundary_is_exclusive`: end_session(t) → `begin_reconnect` at t+1시간-1ms Ok 인 복사본과, t+1시간 에서 `Err(Expired)`; candidates at t+1시간 비어 있음.
  - `reconnect_restarts_the_hour`: end(t) → begin(t+50분) → end(t+100분) → begin(t+100분+59분) Ok.
  - `wrong_device_and_unknown`: 다른 기기 key → `Err(WrongDevice)`, 없는 번호 → `Err(Unknown)`, 세션 중 → `Err(InSession)`.
  - `revoke_and_revoke_all_keep_known`: revoke 뒤 begin → `Err(Unknown)`, `is_known` 은 true.
  - `has_valid_and_prune`, `viewer_book_store_replace_and_expired`, `books_roundtrip_serde` (postcard 왕복 같음).
- [ ] **Step 2: 실패 확인** — `cargo test -p auth permit`.
- [ ] **Step 3: 구현.**
- [ ] **Step 4: 통과 확인** — `cargo test -p auth`.
- [ ] **Step 5: commit** — "feat: 재접속 허가증 장부".

---

### Task 5: `crates/auth` 저장 파일 (`StateFile`, `Protector`)

**Files:**
- Create: `crates/auth/src/store.rs`
- Modify: `crates/auth/src/lib.rs`, `crates/auth/Cargo.toml` (`[features] insecure-dev-store = []`)

**Interfaces:**
- Produces:
  - `auth::StoreError { Io(std::io::Error), Corrupt, Protect(String) }`
  - `trait auth::Protector { fn protect(&self, plain: &[u8]) -> Result<Vec<u8>, StoreError>; fn unprotect(&self, blob: &[u8]) -> Result<Zeroizing<Vec<u8>>, StoreError>; }`
  - `auth::StateFile<P: Protector>`: `new(path: PathBuf, protector: P) -> Self`, `load<T: DeserializeOwned>(&self) -> Result<Option<T>, StoreError>` (파일 없음 → `Ok(None)`), `save<T: Serialize>(&self, value: &T) -> Result<(), StoreError>`
  - 파일 형식: `b"SRS1"` || `protect(postcard(value))`. 머리가 다르거나 풀리지 않거나 해독이 안 되면 `Corrupt`. 평문 postcard buffer 는 `Zeroizing`.
  - 저장: 같은 폴더의 `<파일>.tmp` 에 쓰고 `sync_all` 뒤 `std::fs::rename` 으로 교체 (Windows 에서 기존 파일을 바꿔 씀). 폴더가 없으면 만든다.
  - `auth::DevPlaintextProtector` (`#[cfg(any(test, feature = "insecure-dev-store"))]`): 그대로 통과. `Debug` 는 `"DevPlaintextProtector(INSECURE)"`.

- [ ] **Step 1: 실패하는 테스트 작성** — `store.rs` tests (임시 폴더는 `std::env::temp_dir()` 아래 고유 이름으로 만들고 끝에 지움):
  - `missing_file_is_none`, `save_then_load_roundtrip` (`HostBook` 과 `String` 값), `save_replaces_atomically` (두 번 저장 → 마지막 값, `.tmp` 없음).
  - `corrupt_file_is_error_and_left_untouched`: 파일 머리를 바꿈 → `load` 가 `Err(Corrupt)`, 파일 bytes 는 그대로. 한 byte 잘린 파일도 `Corrupt`.
  - `protector_failure_is_error`: 늘 `Err(Protect)` 를 내는 test protector → `save` 가 `Err`, 파일을 만들지 않음.
- [ ] **Step 2: 실패 확인** — `cargo test -p auth store`.
- [ ] **Step 3: 구현.**
- [ ] **Step 4: 통과 확인** — `cargo test -p auth`. 그리고 feature 없는 일반 빌드에 평문 protector 가 없는지 확인: `cargo build -p auth` 가 성공하고, `grep -n "insecure-dev-store" crates/auth/src/store.rs` 로 `DevPlaintextProtector` 정의가 `cfg(any(test, feature = "insecure-dev-store"))` 아래에만 있음.
- [ ] **Step 5: commit** — "feat: 암호화 저장 파일 StateFile".

---

### Task 6: `crates/platform-win` DPAPI 와 Windows CI

**Files:**
- Create: `crates/platform-win/Cargo.toml`, `crates/platform-win/src/lib.rs`, `crates/platform-win/src/dpapi.rs`, `.github/workflows/windows-platform.yml`
- Modify: 루트 `Cargo.toml` (`platform-win = { path = "crates/platform-win" }`, `windows = { version = "=0.62.2", features = ["Win32_Foundation", "Win32_Security_Cryptography"] }`)

**Interfaces:**
- Produces:
  - `platform_win::DpapiProtector` (`#[cfg(windows)]`, `new() -> Self`): `auth::Protector` 구현. `CryptProtectData(data, None, Some(&entropy), None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out)`, entropy = `b"simple-remote store v1"`, `CryptUnprotectData` 도 같은 entropy. 결과 buffer 는 `LocalFree`. 실패 → `StoreError::Protect(HRESULT 문자열)`. `CRYPTPROTECT_LOCAL_MACHINE` 은 쓰지 않는다.
  - Windows 가 아닌 대상에서는 crate 가 비어 있다 (`lib.rs` 의 모듈이 `#[cfg(windows)]`).
- workflow: `on: push paths ['crates/platform-win/**', '.github/workflows/windows-platform.yml'], workflow_dispatch`. job `windows-2025`: toolchain 1.95, `cargo test -p platform-win`, `cargo build --workspace --all-features` (제품 crate 전체가 Windows 에서 빌드되는지 확인. D24 와 같게 경로 조건으로 Windows 분을 아낌).

- [ ] **Step 1: 실패하는 테스트 작성** — `dpapi.rs` 의 `#[cfg(all(test, windows))]` tests: `roundtrip` (protect → unprotect 같음, blob 이 평문을 포함하지 않음), `tampered_blob_fails` (중간 byte 뒤집기 → `Err(Protect)`), `state_file_with_dpapi` (`auth::StateFile::new(tmp, DpapiProtector::new())` 로 `HostBook` 저장·읽기).
- [ ] **Step 2: 실패 확인** — WSL 에서는 Windows 대상 테스트가 돌지 않으므로 `cargo build --workspace` 가 Linux 에서 깨지지 않는지만 확인하고, 브랜치를 push 해 Actions 의 `windows-platform` 실행에서 컴파일 실패(모듈 없음)를 확인한다.
- [ ] **Step 3: 구현** — 사용법은 `spikes/win-probes/src/bin/dpapi_probe.rs` (entropy 인자만 추가).
- [ ] **Step 4: 통과 확인** — push 후 `gh run watch` 로 `windows-platform` 이 `cargo test -p platform-win` 3 passed 와 workspace 빌드 성공.
- [ ] **Step 5: commit** — "feat: DPAPI 저장 보호와 Windows 빌드 확인".

---

### Task 7: 재접속 흐름과 허가증 발급

**Files:**
- Create: `crates/protocol/src/reconnect_msg.rs`, `crates/viewer-core/src/reconnect.rs`, `apps/host-agent/tests/reconnect.rs`
- Modify: `crates/protocol/src/{lib.rs,control.rs}`, `crates/viewer-core/src/{lib.rs,connect.rs}`, `apps/host-agent/src/{lib.rs,session.rs}`, `apps/host-agent/tests/first_connection.rs`

**Interfaces:**
- Consumes: Task 2~4.
- Produces:
  - `protocol::reconnect_msg::{ReconnectMsg, encode, decode}`: `enum ReconnectMsg { Start { noise1: Vec<u8> }, Accept { noise2: Vec<u8> }, Offer { sealed: Vec<u8> }, Answer { sealed: Vec<u8> }, Refused }` (relay data = hex(postcard), `PakeMsg` 와 같은 방식)
  - `Control::Permit { id: Vec<u8> }` (16 byte), `RejectReason::PermitInvalid`
  - host: `HostState { pub code, pub limiter, pub book: HostBook }`. `ApprovalRequest` 에 `pub known_before: bool` (`book.is_known`). `HostSession { pub viewer, pub permit_id: [u8; 16], pub reconnected: bool, .. }`. `HostSession::serve_pings(&mut self, for_at_most: Duration, stop: &mut dyn FnMut() -> bool) -> Result<SessionEnd, HostError>`, `enum SessionEnd { PeerClosed, Stopped, TimedOut }` (`stop` 이 true 면 연결을 닫고 `Stopped`). 세션이 끝난 뒤 호출자가 `PeerClosed | TimedOut` 이면 `book.end_session`, `Stopped`(가족 끊기) 면 `book.revoke`.
  - 새 연결 성공(Accepted 전송 직후): `book.issue(&viewer)` → `Control::Permit { id }` 전송.
  - `serve_next` 의 `kind: Reconnect` 흐름 (각 대기는 `signaling_step`):
    1. `Start { noise1 }` 수신 → `HostReconnect::respond(keys, host_id, noise1, &book.reconnect_candidates(now))`. 실패 → relay `Refused` + `Kick { n }` → `Failed(AttemptFailure::PermitInvalid)` (주소 없음).
    2. 성공 → relay `Accept { noise2 }` → `Offer { sealed }` 를 `channel.open` → `accept_offer` → relay `Answer { sealed: channel.seal(answer) }`.
    3. data channel 이 열리면 Hello 교환 (binding = `session.binding`). viewer Hello 의 `device_key`, `reconnect_key` 가 허가증의 viewer 와 다르면 `Rejected(BadIdentity)`. 버전 확인은 첫 연결과 같음.
    4. `book.begin_reconnect(id, device_key, now)` 실패 → `Rejected(PermitInvalid)` → `Failed(PermitInvalid)`. 성공 → `Decision::Accepted` (허락 묻지 않음, 코드 바꾸지 않음) → `Session(HostSession { reconnected: true, .. })`.
  - `AttemptFailure::PermitInvalid` 추가. 재접속 실패는 코드 추측 제한(`limiter`)에 넣지 않는다 (코드가 없는 경로).
  - viewer: `connect` 는 `Decision::Accepted` 뒤 `Control::Permit` 을 받아 `ViewerSession { pub host, pub permit_id: Option<[u8; 16]>, .. }` 로 돌려준다 (없거나 길이가 틀리면 `Security("permit")`).
  - `viewer_core::reconnect<L: Link<ServerToViewer, ViewerToServer>>(link: L, permit: &ViewerPermit, keys: &DeviceKeys, bind_ips: &[IpAddr], t: &Timeouts) -> Result<ViewerSession, ConnectError>` (순서는 위 1~4 의 viewer 쪽, host Hello 의 `device_key`/`reconnect_key` 가 `permit.host` 와 다르면 `Security("host identity")`). `Refused` 나 `Rejected(PermitInvalid)` → `ConnectError::PermitRejected`.
  - `viewer_core::retry_delays() -> impl Iterator<Item = Duration>`: 1, 2, 5, 10, 30초, 이후 60초 반복.

- [ ] **Step 1: 실패하는 테스트 작성** — `apps/host-agent/tests/reconnect.rs` (첫 연결 테스트의 도우미 방식, host thread 가 `serve_next` 를 두 번 부르고 사이에 `book.end_session`):
  - `permit_issued_then_reconnect_without_approval`: 첫 연결 → viewer `permit_id` Some, host `book` 에 `Session` → 세션 끝(`serve_pings` 가 viewer drop 으로 `PeerClosed`) → `end_session` → viewer `reconnect` (host 의 `approve` 는 호출되면 panic) → 성공, ping, host 결과 `Session { reconnected: true }`, 코드 그대로.
  - `revoked_permit_is_refused`: revoke 뒤 reconnect → `Err(ConnectError::PermitRejected)`, host relay 기록에 `ReconnectMsg::Accept`/`Answer` 없음.
  - `expired_permit_is_refused`: `Waiting { since_ms }` 를 1시간 1초 전으로 만든 장부 → `PermitRejected`.
  - `copied_permit_id_with_other_keys_is_refused`: 다른 `DeviceKeys` 로 같은 `ViewerPermit` 을 씀 → `PermitRejected`, host 가 `Answer` 를 보내지 않음.
  - `permit_in_session_is_not_a_candidate`: 세션이 끝나기 전 두 번째 reconnect → `PermitRejected`.
  - `known_before_on_second_new_connection`: 같은 기기로 새 연결 두 번 → 두 번째 `ApprovalRequest.known_before == true`, 첫 번째 false.
  - `crates/viewer-core` unit: `retry_delays` 의 앞 7개가 `[1, 2, 5, 10, 30, 60, 60]` 초.
- [ ] **Step 2: 실패 확인** — `cargo test -p host-agent --test reconnect`. 기대: 컴파일 실패.
- [ ] **Step 3: 구현** — viewer·host 의 봉인 SDP·Hello·Decision 처리는 첫 연결 코드와 공유하고 복사하지 않는다 (예: host `handshake` 가 binding 과 "허락 또는 허가증 확인" 단계를 인자로 받게).
- [ ] **Step 4: 통과 확인** — `cargo test --workspace --all-features` 두 번.
- [ ] **Step 5: commit** — "feat: 재접속 허가증으로 허락 없이 다시 연결".

---

### Task 8: 콘솔 실행 파일의 저장·재접속과 end-to-end

**Files:**
- Create: `apps/host-agent/src/data.rs`, `crates/viewer-core/src/data.rs`
- Modify: `apps/host-agent/src/main.rs`, `apps/host-agent/Cargo.toml`, `apps/viewer/src/main.rs`, `apps/viewer/Cargo.toml`, `crates/viewer-core/src/lib.rs`, `tools/e2e-local.sh`, 진행 기록

**Interfaces:**
- Produces:
  - `host_agent::HostData { pub device: Vec<u8> /* DeviceKeys::to_secret_bytes */, pub host_id: Option<String>, pub book: HostBook }` (serde), 파일 `<data-dir>/host.bin`
  - `viewer_core::ViewerData { pub device: Vec<u8>, pub book: ViewerBook }` (serde), 파일 `<data-dir>/viewer.bin`
  - 두 실행 파일의 protector: Windows 는 `platform_win::DpapiProtector`, 그 밖은 feature `insecure-dev-store`(각 app 의 feature 가 `auth/insecure-dev-store` 를 켬)일 때 `DevPlaintextProtector`, 아니면 "이 빌드는 저장소를 지원하지 않습니다" 를 stderr 에 쓰고 종료 코드 2.
  - 기본 데이터 폴더: host `%LOCALAPPDATA%\SimpleRemote\host`, viewer `%APPDATA%\SimpleRemote\viewer` (Windows), 그 밖은 `$HOME/.local/share/simple-remote/{host,viewer}`. `--data-dir <dir>` 로 바꿈.
  - host-agent: `host-agent --server <url> [--name <이름>] [--data-dir <dir>] [--reconnect-only] [--once] | host-agent --revoke-all [--data-dir <dir>]`.
    - 시작: 저장 파일을 읽음 (깨졌으면 "저장 파일을 읽을 수 없습니다" + 종료 코드 1, 파일을 건드리지 않음). 없으면 새 기기 key. `register_host(server, keys, host_id)` → 새 ID 면 저장. `ID <id>` 출력.
    - 기본: `set_mode(New)`, `CODE` 출력(연결 뼈대와 같음). `--reconnect-only`: 유효 허가증이 없으면 "재접속 허가가 없습니다" + 종료 코드 1, 있으면 `set_mode(Reconnect)` 이고 `CODE` 를 출력하지 않음.
    - 허락 질문: `APPROVE? <이름> <지문> <처음|이전> [y/N]` (`known_before` 로 `처음`/`이전`).
    - 세션 시작: 새 연결이면 `CONNECTED <이름> <지문>`, 재접속이면 `RECONNECTED <이름> <지문>`. 세션 중 stdin 에 `x` 한 줄 → 끊고 허가증 폐기, `ENDED family`. viewer 가 닫으면 `ENDED peer` 와 `end_session`. 장부가 바뀔 때마다 저장.
    - `--revoke-all`: 장부의 허가증을 모두 지우고 저장, `REVOKED <개수>`, 종료 코드 0.
  - viewer: `viewer --server <url> --id <9자리> (--code <6자리> | --reconnect) [--name] [--data-dir]`.
    - 새 연결 성공 → 허가증을 `ViewerBook` 에 저장.
    - `--reconnect`: 저장된 허가증이 없거나 만료면 "재접속 허가가 없습니다" 종료 코드 1. 있으면 `?kind=reconnect` 로 `reconnect`, 실패가 `PermitRejected` 면 허가증을 지우고 "재접속 허가가 취소되었거나 만료되었습니다" 종료 코드 1. `HostNotWaiting`·`HostLeft`·`Timeout` 이면 `retry_delays` 간격으로 만료 때까지 다시 시도 (시도마다 `RETRY <초>` 한 줄).
    - 성공 출력: 새 연결 `CONNECTED ...`, 재접속 `RECONNECTED <host 이름> <지문>`, 이어서 `PONG` 3줄. 끝나면 `mark_disconnected` 저장.
- e2e (`tools/e2e-local.sh`, 두 실행 파일을 `--features insecure-dev-store` 로 빌드, 데이터 폴더는 `mktemp -d`):
  1. host(기본 모드, `--once` 없이) 시작 → 틀린 코드 실패 → 맞는 코드 `CONNECTED` + `PONG` 3 → host `ENDED peer`.
  2. `viewer --reconnect` → `RECONNECTED` + `PONG` 3, host 는 `APPROVE?` 없이 `RECONNECTED`.
  3. host 종료 → 같은 데이터 폴더로 `--reconnect-only` 재시작 → 같은 `ID`, `CODE` 줄 없음 → `viewer --reconnect` 성공.
  4. 다시 `viewer --reconnect` 연결 중 host stdin 에 `x` → host `ENDED family` → `viewer --reconnect` 가 "취소되었거나 만료" 메시지와 종료 코드 1.
  5. host 저장 파일의 첫 byte 를 바꿔 시작 → 종료 코드 1, 파일 bytes 그대로.
  6. `E2E OK`.

- [ ] **Step 1: 실패하는 e2e 작성** — 위 1~6 을 `tools/e2e-local.sh` 에 추가 (기존 단계 유지).
- [ ] **Step 2: 실패 확인** — `bash tools/e2e-local.sh`.
- [ ] **Step 3: 구현** — `data.rs` 두 개, 두 `main.rs`. stdin 은 한 thread 가 줄 단위로 읽어 허락 질문과 세션 중 `x` 에 나눠 준다.
- [ ] **Step 4: 통과 확인** — `bash tools/e2e-local.sh` → `E2E OK`, `cargo test --workspace --all-features`, `cd signaling && npm test`.
- [ ] **Step 5: 진행 기록 갱신** — "현재 상태와 인계", "다음 할 일" (다음 후보: Windows 서비스 구조 + SYSTEM 폴더 전환 + 재부팅 후 재접속, 또는 화면·입력).
- [ ] **Step 6: commit** — 코드 "feat: 콘솔 실행 파일의 저장과 재접속", 문서 "docs: 재접속 계획 완료 기록".

---

## spec 덮임 점검

| spec 요구 | task |
|---|---|
| 4절 host·viewer 기기 key, 재접속 key 저장, DPAPI (LOCAL_MACHINE 금지) | 3 (직렬화), 5, 6, 8 |
| 4절 접속 ID 는 host 공개키에 묶이고 유지 | 8 (저장한 ID 로 `register_host`) |
| 4절 알고 있는 상대 (viewer: host ID·공개키·이름, host: viewer 공개키·이름) | 4 (`HostBook.known`, `ViewerPermit.host`), 7 |
| 5.1 서버가 허가증을 보지 못함, host 모드(새 연결/재접속만) | 1, 3 (번호는 Noise payload), 8 |
| 5.2 6단계 처음 보는 기기인지 이전 기기인지 표시 | 7 (`known_before`), 8 |
| 5.3 허가증 필드·상태·만료·재시작 | 4 |
| 5.3 재접속 흐름 1~4 (Noise KK, 허가증·key 확인, 봉인 SDP, 허락 없이 시작, 재접속됨 표시) | 3, 7, 8 |
| 5.3 자동 재시도 간격, 스스로 끊으면 재시도 안 함 | 7 (`retry_delays`), 8 |
| 5.3 폐기: 가족 끊기, 1시간, 모두 취소 | 4, 8 |
| 5.5 허가증 도용(번호만으로 불가) | 3, 7 (`copied_permit_id_with_other_keys_is_refused`) |
| 5.5 노출 시간 (유효 허가증이 있을 때만 재접속만 등록) | 8 (`--reconnect-only` 와 `has_valid`) |
| 8.1 허가증 무효·만료·폐기·viewer key 불일치 거절 | 4, 7 |
| 10.4 폐기·만료 허가증 거부, 다른 기기 key 로 허가증 사용 거부 | 7 |
| 연결 뼈대에서 미룬 Kick 경쟁 | 1, 2 |

다음 계획으로 넘긴 spec 요구: 5.3 "재부팅 직후 로그인 화면에서 재접속만 받기"(서비스 필요), 8.3 순단 대비, 5.6 접속 기록, 7.1 제거 시 저장소 삭제.

빈칸 점검: 이 문서에서 "TBD", "나중에", "적절히" 를 검색해 0건.
