# SP1 Phase 0 구현 계획: spike

- 작성일: 2026-09-23
- 상태: 사용자 승인됨, 실행 중
- spec: `docs/superpowers/specs/2026-09-24-simple-remote-sp1-design.md` (11절 spike 목록이 이 계획의 범위)
- 진행 기록: `docs/superpowers/progress/2026-09-23-remote-control-brainstorming.md`

## 목표

spec 11절의 확인 항목 9개에 실제 코드와 실행 결과로 답하고, 그 결과로 WebRTC·PAKE·재접속 key 합의 라이브러리를 고르고 Windows 쪽 가정(SYSTEM DPAPI, SYSTEM agent 의 하드웨어 인코더, `SendInput` 좌표, egui, WiX)을 확정한다.

## 접근 방식

spike 마다 "질문 → 통과/실패 기준 → 검사 코드 → 실행 → 결과 기록"을 한 task 로 묶는다. Linux 에서 되는 것은 클라우드 세션이 돌리고, Windows 빌드와 자동 검사는 GitHub Actions(`windows-2025` runner)가 돌리며, 사람이 봐야 하는 것(GPU, 입력, 화면, 한글 IME)만 사람이 Windows PC 에서 실행한다. spike 코드는 버릴 코드로 `spikes/` 아래에 따로 두고 제품 코드(`crates/`, `apps/`)에서 참조하지 않는다.

## 사용 기술 (확인한 버전)

| 영역 | 후보 | 버전 | 확인 방법 |
|---|---|---|---|
| WebRTC | `str0m` (sans-IO) | 0.23.1, feature `aws-lc-rs` | crate 소스, 메모리 연결 예제 실행 |
| WebRTC | `webrtc` (webrtc-rs, tokio) + `rtc` | 0.21.0 | crate 소스, loopback 예제 실행 |
| PAKE | `pakery-spake2` + `pakery-crypto` (`p256`, `spake2`) | 0.6.0 | crate 소스, RFC 9382 vector 1 재현 |
| PAKE | `spake2` (Ed25519, RFC 이전 draft) | 0.4.0 | crate 소스, python-spake2 vector 재현 |
| Noise KK | `snow` | 0.10.0 | crate 소스, KK 왕복 테스트 |
| Windows API | `windows` | 0.62.2 | crate 소스, `cargo check --target x86_64-pc-windows-msvc` |
| 서비스 | `windows-service` | 0.8.1 | crate 소스(`examples/ping_service.rs`) |
| UI | `eframe` / `egui` | 0.36.2 (Rust 1.95 이상 필요) | crate 소스, Linux 빌드 |
| signaling 테스트 | `@cloudflare/vitest-plugin` 1.2.4, vitest 4.1.11, wrangler 4.137.0 | | npm tarball, 로컬 테스트 4건 통과 |
| 설치 | WiX Toolset (`wix` dotnet tool) + Util, Firewall 확장 | 7.0.0 | nupkg, WiX 소스 xsd |
| CI | GitHub Actions `windows-2025`, `actions/checkout@v7`, `actions/upload-artifact@v7` | | runner-images 문서, 릴리스 목록 |

`@cloudflare/vitest-pool-workers` 는 1.0 에서 `@cloudflare/vitest-plugin` 으로 이름이 바뀌었다. vitest 5 는 plugin 의 peerDependency(`^4.1.0`) 밖이다.

## spec 에서 옮긴 전역 제약

| 항목 | 값 | spec 절 |
|---|---|---|
| 전송 | WebRTC (ICE, DTLS, SRTP/RTP, data channel), TURN 없음, relay 없음 | 3.3, 2.3 |
| STUN | `stun.cloudflare.com:3478` | 3.3 |
| 일회용 코드 | 숫자 6자리, PAKE 를 SDP 교환보다 먼저, key 확인 MAC 까지 끝나야 다음 단계 | 5.2 |
| 기기 key | Ed25519. host 는 SYSTEM 계정 범위 DPAPI, `CRYPTPROTECT_LOCAL_MACHINE` 금지 | 4 |
| 재접속 key 합의 | Noise KK 패턴 후보 | 5.3 |
| 코덱 | H.264 만, MF 하드웨어 MFT 우선, 내장 소프트웨어 MFT 대체, B-frame 없음 | 6.2 |
| 목표 화질 | 1920x1080, 30fps, 캡처부터 표시까지 처리 지연 100ms 이하 (설계 제안 목표) | 6.2, 10 |
| 순단 단계 | heartbeat 500ms, 1초 키·버튼 해제, 2초 불안정 표시, 5초 ICE restart, ICE restart 20초 실패 시 허가증 재접속 (설계 제안값) | 8.3 |
| 좌표 | host 는 Per-Monitor v2, physical pixel virtual desktop 좌표(음수 가능), `SendInput` 절대 좌표 0..65535 + `MOUSEEVENTF_VIRTUALDESK`, 주입 후 `GetCursorPos` 로 1픽셀 확인 | 6.3 |
| 지원 OS | Windows 11 23H2 이상, Windows 10 22H2 | 6.6 |
| 설치 | WiX MSI, LocalSystem 서비스 자동 시작·실패 시 재시작, host-agent 한정 방화벽 규칙, `ProgramData\SimpleRemote` SYSTEM 전용, 제거 시 흔적 0건 | 7.1, 7.3 |
| signaling | Cloudflare Workers + Durable Objects 무료 plan, TypeScript, `wss://` | 3.3, 5.1 |
| 보안 경로 | 실패하면 막는다. 암호화 없이 진행하는 코드 경로 없음 | 8.1 |

## 이 계획에서 새로 정한 것 (검토 요청)

1. **Windows 빌드 방식**: GitHub Actions `windows-2025` runner 로 빌드하고 자동 검사한다. 사람 PC 에는 빌드 도구를 설치하지 않고, Actions 가 만든 exe 를 받아 실행만 한다 (사용자 승인, D22). 근거: 이 컨테이너는 proxy 가 Microsoft 다운로드를 막아 Windows exe 를 링크할 수 없다 (`cargo xwin` 이 `aka.ms` 403 으로 실패). runner 이미지에는 Rust 1.98.1, VS 2022 MSVC, Windows SDK 10.0.26100, .NET SDK 10 이 있다.
2. **Actions 사용량**: 조직은 GitHub Free plan (사용자 답변). private 저장소는 한 달 2,000분이고 Windows 는 2배로 차감되어 약 1,000분이다. workflow 는 `spikes/**` 변경과 수동 실행에서만 돈다.
3. **WebRTC 선택 규칙** (Task 7): 5개 기능 테스트 통과 수 → Windows 빌드 성공 → 둘 다 같으면 `str0m`. 동점일 때 `str0m` 을 고르는 근거: 소스 확인에서 webrtc-rs 0.21 은 (a) 비동기 API 로 내 쪽 후보(UPnP 매핑 주소)를 추가할 수 없어 SDP 문자열을 고쳐야 하고 (b) PLI 를 보내는 쪽 앱이 받으려면 직접 만든 interceptor 가 필요하며 (c) 대역폭 추정이 기본으로 꺼져 있고 값을 꺼내는 API 가 없으며 (d) 한 달 사이 사전 릴리스 5번으로 API 가 자주 바뀐다. `str0m` 은 sans-IO 라 spec 9.1 의 연결 통합 테스트·NAT 흉내 테스트를 가짜 네트워크로 돌리기 쉽다.
4. **결과 기록 위치**: `docs/research/2026-09-23-sp1-phase0-spike-results.md` (Task 0 에서 뼈대 작성). 각 task 는 자기 절만 채운다.

## spike 공통 규칙

- spike 코드는 버릴 코드다. `spikes/README.md` 에 이 사실을 적고, 각 파일 첫 줄 주석에 `Spike only (throwaway)` 를 적는다. 제품 코드는 `spikes/` 를 참조하지 않는다. 제품 구현 때 필요한 부분은 새로 쓴다.
- spike 는 각자 독립 Cargo package(자기 `Cargo.lock`)다. 공용 workspace 를 만들지 않는다. 근거: Windows 전용 crate 와 Linux 테스트 crate 가 섞이지 않고, 버릴 때 폴더만 지우면 된다.
- Rust toolchain 은 `spikes/rust-toolchain.toml` 로 1.95 고정 (egui 0.36 최소 요구). rustup 은 현재 디렉터리와 상위 디렉터리에서 이 파일을 찾으므로 명령은 각 spike 폴더 안에서 실행한다.
- spike 테스트는 제품 코드가 아니라 라이브러리 동작을 확인한다. 그래서 TDD 순서를 이렇게 적용한다: 테스트를 먼저 쓰고 → 공용 harness 없이 실행해 컴파일 실패를 확인 → harness 를 넣고 → 실행한다. 이때 assertion 이 실패하면 추측으로 고치지 않는다. 원인을 찾아 harness 문제인지 라이브러리 한계인지 가리고, 라이브러리 한계면 결과 문서에 "실패"로 기록한다 (그것이 spike 의 답이다).
- 계획의 코드는 작성 시점에 scratchpad 에서 컴파일만 확인했다 (`cargo test --no-run`, Windows 는 `cargo check --target x86_64-pc-windows-msvc`). 테스트 실행은 승인 뒤 각 task 에서 한다. 예외: Task 8 (Workers) 과 Task 2 의 RFC vector 테스트는 API 확인 과정에서 이미 한 번 실행해 통과했다.
- 부록 코드는 `부록 X` 로 참조한다. 부록의 파일 내용을 그대로 만든다. 계획 작성 시 부록에서 파일을 다시 꺼내 빈 폴더에서 Rust 1.95 로 컴파일해 부록과 컴파일한 코드가 같음을 확인했다 (Linux `cargo test --no-run` 4개 package, Windows 대상 `cargo check` 3개 package, 경고 0).
- 각 Rust spike 의 `Cargo.lock` 은 첫 빌드에서 생기며 함께 commit 한다 (Actions 와 클라우드가 같은 의존성 버전을 쓰게).

## 사람 준비 사항 (Windows task 전에)

| 번호 | 할 일 | 필요한 task |
|---|---|---|
| H1 | Windows spike 를 돌릴 PC 결정. 회사 관리 PC 가 아닌 개인 PC 권장 (SYSTEM 권한 실행과 입력 주입을 하므로). 보류 중인 판단 | 10, 11, 12 |
| H2 | 그 PC 의 정보 알려 주기: Windows 버전(`winver`), GPU 종류, 모니터 수와 각 모니터 배율(설정 → 디스플레이) | 10, 11, 12 |
| H3 | PsExec 받기: https://download.sysinternals.com/files/PSTools.zip 을 풀어 `PsExec64.exe` 를 `C:\spike\` 에 둔다 | 11 |
| H4 | GitHub 에서 Actions artifact 받는 법: 저장소 → Actions 탭 → `windows-spikes` 실행 하나 → 아래 Artifacts 에서 `win-spikes` 받기 → `C:\spike\` 에 풀기 | 10, 11, 12 |

인터넷에서 받은 exe 는 Windows 가 막을 수 있다. 푼 뒤 PowerShell 에서 `Get-ChildItem C:\spike -Recurse | Unblock-File` 을 한 번 실행한다.

결과 보고 방법(모든 Windows 사람 task 공통): 명령 출력 전체를 복사해 대화창에 붙여 넣는다. 파일로 남기는 명령은 그 파일 내용을 붙여 넣는다. 에이전트가 결과 문서에 옮긴다.

---

## Task 0. spike 뼈대와 결과 문서

- 실행 위치: 클라우드
- 파일
  - 만들기: `spikes/README.md`, `spikes/rust-toolchain.toml`, `spikes/.gitignore`, `docs/research/2026-09-23-sp1-phase0-spike-results.md`
- 다른 task 와 주고받는 것: 결과 문서의 절 제목 `## T<번호>` (각 task 가 채움), toolchain 1.95

- [ ] **1단계: 파일 작성**

`spikes/README.md`:

```markdown
# spikes (버릴 코드)

SP1 Phase 0 spike 코드. 라이브러리 선택과 Windows 가정 확인용이며 제품 코드가 아니다.
제품 코드(`crates/`, `apps/`)는 이 폴더를 참조하지 않는다. Phase 0 결과가 반영되면 폴더째 지운다.
계획: `docs/superpowers/plans/2026-09-23-sp1-phase0-spikes.md`
결과: `docs/research/2026-09-23-sp1-phase0-spike-results.md`

명령은 각 spike 폴더 안에서 실행한다 (`rust-toolchain.toml` 적용 때문).
```

`spikes/rust-toolchain.toml`:

```toml
[toolchain]
channel = "1.95"
```

`spikes/.gitignore`:

```
target/
node_modules/
*.msi
*.wixpdb
```

`docs/research/2026-09-23-sp1-phase0-spike-results.md`:

```markdown
# SP1 Phase 0 spike 결과

계획: `docs/superpowers/plans/2026-09-23-sp1-phase0-spikes.md`. 각 절은 해당 task 가 채운다.
형식: 질문 / 기준 / 실행 환경(OS, 버전, 명령) / 출력 요약 / 판정(통과·실패·부분) / 설계 영향.

## T1 GitHub Actions Windows 빌드
## T2 PAKE
## T3 재접속 key 합의 (Noise KK)
## T4 WebRTC: str0m
## T5 WebRTC: webrtc-rs
## T6 WebRTC: Windows 빌드와 IPv6
## T7 WebRTC 라이브러리 결정
## T8 Cloudflare Workers 로컬 테스트
## T9 SYSTEM 계정 DPAPI
## T10 SendInput 절대 좌표
## T11 MF 하드웨어 인코더 (SYSTEM agent)
## T12 egui
## T13 WiX MSI
## 종합
```

- [ ] **2단계: 확인**

실행: `cd spikes && rustup toolchain install 1.95 --profile minimal && rustc --version`
기대: `rustc 1.95.` 로 시작하는 줄.

- [ ] **3단계: commit**

```bash
git add spikes/README.md
git add spikes/rust-toolchain.toml
git add spikes/.gitignore
git add docs/research/2026-09-23-sp1-phase0-spike-results.md
git commit  # 메시지: "chore: SP1 Phase 0 spike 뼈대와 결과 문서 추가" (CLAUDE.md 형식)
```

---

## Task 1. GitHub Actions Windows 빌드

- 실행 위치: 클라우드(작성) + GitHub Actions(실행)
- 질문: 이 저장소에서 Actions 가 켜져 있나? `windows-2025` 에서 Rust spike 를 빌드·실행할 수 있나? 한 번에 몇 분 걸리나?
- 기준
  - 통과: workflow 가 실행되고, `winprobe` 빌드와 `dpapi_probe` 자기 계정 왕복이 성공하며, artifact `win-spikes` 가 올라간다.
  - 실패: 실행이 시작되지 않음(조직 설정에서 Actions 꺼짐) 또는 빌드 실패. 원인을 로그로 확인해 기록한다.
- 파일
  - 만들기: `.github/workflows/windows-spikes.yml` (부록 A), `spikes/win-probes/Cargo.toml`, `spikes/win-probes/src/bin/dpapi_probe.rs`, `spikes/win-probes/src/bin/sendinput_probe.rs`, `spikes/win-probes/src/bin/mf_probe.rs` (부록 B)
- 다른 task 와 주고받는 것: artifact 이름 `win-spikes` (Task 10, 11, 12 가 사람에게 받게 함), job 이름 `build-test`, 로그 줄 `RESULT ...` 형식 (Task 9, 13 이 추가하는 step 이 같은 형식으로 출력)

이 task 이후 Task 6, 9, 12, 13 이 같은 workflow 에 step 을 더한다. 이 task 에서는 부록 A 중 `# Task 1` 표시된 step 만 넣는다.

- [ ] **1단계: 실패 확인**

GitHub 도구로 `list_workflows` 실행. 기대: `total_count: 0` (workflow 없음).

- [ ] **2단계: 파일 작성**

부록 A 의 Task 1 부분, 부록 B 전체.

- [ ] **3단계: 클라우드에서 컴파일 확인**

```bash
cd spikes/win-probes
rustup target add x86_64-pc-windows-msvc
cargo check --target x86_64-pc-windows-msvc --bins
```

기대: `Finished`, error 0, warning 0.

- [ ] **4단계: commit, push, 실행 확인**

```bash
git add .github/workflows/windows-spikes.yml
git add spikes/win-probes/Cargo.toml
git add spikes/win-probes/src/bin/dpapi_probe.rs
git add spikes/win-probes/src/bin/sendinput_probe.rs
git add spikes/win-probes/src/bin/mf_probe.rs
git commit  # "chore: Windows spike 빌드용 GitHub Actions workflow 추가"
git push -u origin <작업 브랜치>
```

GitHub 도구로 `list_workflow_runs` → 실행 id → `list_workflow_jobs` → `get_job_logs`.
기대: `dpapi roundtrip RESULT ok=true user=runneradmin` 줄, `mf_probe` 의 `RESULT` 줄(하드웨어 인코더 없음이 정상, 소프트웨어 결과는 참고), job 소요 시간.

- [ ] **5단계: 결과 기록**

결과 문서 `## T1` 에 실행 URL, 소요 시간(분), cache 없을 때 빌드 시간, runner 계정 이름, `mf_probe` 출력의 인코더 목록을 적는다. 한 달 1,000분 기준 가능한 실행 횟수를 계산해 적는다.

```bash
git add docs/research/2026-09-23-sp1-phase0-spike-results.md
git commit  # "docs: Phase 0 Actions 빌드 결과 기록"
```

---

## Task 2. PAKE

- 실행 위치: 클라우드
- 질문
  1. RFC 9382 테스트 값을 재현하는 Rust SPAKE2 구현이 있나?
  2. 틀린 코드를 key 확인 MAC 단계에서 거부하나?
  3. `spake2` 0.4.0 (spec 5.7 의 후보) 은 RFC 9382 를 따르나?
- 기준
  - 통과: `pakery_rfc9382` 3건 통과 (RFC 9382 Appendix B vector 1 의 pA, pB, K, Hash(TT), Ke, KcA, KcB, MAC_A, MAC_B 전부 일치 + 틀린 코드 `ConfirmationFailed`).
  - `spake2_crate` 5건은 비교 자료다. 이 crate 가 RFC 9382 가 아니라는 사실(P-256 없음, key 확인 MAC 없음, 다른 transcript)은 소스로 이미 확인했고, 테스트는 그 동작(틀린 코드에서도 `finish` 성공, 다른 key)을 고정해 보여 준다.
  - `opaque-ke` 는 비밀번호 등록 기록을 서버에 두는 aPAKE 라 매번 새로 만드는 코드와 맞지 않는다. 코드 비교 없이 결과 문서에 근거만 기록한다 (`src/opaque.rs` 등록·로그인 흐름).
- 파일
  - 만들기: `spikes/auth/Cargo.toml`, `spikes/auth/src/lib.rs`, `spikes/auth/tests/pakery_rfc9382.rs`, `spikes/auth/tests/spake2_crate.rs` (부록 C)
- 다른 task 와 주고받는 것: 없음. 결과는 이후 `crates/auth` 계획의 입력.

- [ ] **1단계: 테스트 작성** — 부록 C 의 `Cargo.toml`, `src/lib.rs`, 테스트 2개.
- [ ] **2단계: 실행**

```bash
cd spikes/auth
cargo test --test pakery_rfc9382 --test spake2_crate
```

기대: `pakery_rfc9382` 3 passed, `spake2_crate` 5 passed.
실패하면: vector 값 불일치 → 입력 scalar 변환(big-endian, `0^32 || s`)부터 확인. 추측으로 값을 바꾸지 않는다.

- [ ] **3단계: 결과 기록** — `## T2` 에 판정, 두 crate 비교표(RFC 준수, 곡선, key 확인, 감사 여부, 관리 상태: pakery 는 1인·별 2·미감사·2026-09 에 0.3→0.6, spake2 는 RustCrypto·미감사·RFC 이전 draft), 의존성 영향(pakery 는 `rand_core` 0.10 / `sha2` 0.11 세대, 다른 세대와 함께 빌드됨을 이 package 에서 확인), 권고안.
- [ ] **4단계: commit** — `spikes/auth/` 파일과 결과 문서를 하나씩 `git add`, 메시지 "test: PAKE spike 추가와 결과 기록".

---

## Task 3. 재접속 key 합의 (Noise KK)

- 실행 위치: 클라우드
- 질문: `snow` 로 Noise KK 상호 인증 key 합의가 되나? 상대 key 가 틀리면 첫 메시지에서 실패하나? spec 4절의 Ed25519 기기 key 를 그대로 쓸 수 있나?
- 기준
  - 통과: `noise_kk` 3건 통과 (왕복 + 양쪽 방향의 틀린 key 가 `snow::Error::Decrypt`).
  - key 종류: `snow` 0.10 의 25519 는 X25519 뿐이다 (`resolvers/default.rs` `Dh25519`). Ed25519 key 를 직접 넣을 수 없다. 결과 문서에 선택지를 적는다: (a) 기기마다 X25519 정적 key 를 따로 두고 Ed25519 로 서명해 묶기 (b) Ed25519 → X25519 변환. 권고는 (a). 근거: 변환 코드를 직접 다루지 않고, `snow` 가 key 길이를 검사하지 않으므로(`default.rs:220-225`, 짧으면 0으로 채움) 길이 검사를 우리 코드에 두기만 하면 된다.
- 파일
  - 만들기: `spikes/auth/tests/noise_kk.rs` (부록 C)
  - 고치기: 없음 (`spikes/auth/Cargo.toml` 에 `snow` 가 이미 있음)
- 다른 task 와 주고받는 것: 없음.

- [ ] **1단계: 테스트 작성** — 부록 C 의 `tests/noise_kk.rs`.
- [ ] **2단계: 실행** — `cd spikes/auth && cargo test --test noise_kk`. 기대: 3 passed.
- [ ] **3단계: 결과 기록** — `## T3` 에 판정, key 종류 선택지와 권고, spec 4절에 "재접속용 X25519 정적 key" 추가가 필요한지 판단.
- [ ] **4단계: commit** — "test: Noise KK spike 추가와 결과 기록".

---

## Task 4. WebRTC: str0m

- 실행 위치: 클라우드
- 질문 (spec 11절 WebRTC 행)
  1. 외부 후보(UPnP 매핑 주소)만 알려도 연결되나?
  2. 같은 세션 안에서 ICE restart 로 viewer 주소가 바뀌어도 data channel 이 이어지나?
  3. H.264 RTP 나누기·재조립이 byte 단위로 같고, 받는 쪽 key frame 요청이 보내는 쪽에 도착하나?
  4. data channel 신뢰(순서 보장)·비신뢰(재전송 0회)가 손실 환경에서 기대대로 동작하나?
  5. 대역폭 추정이 병목 1 Mbps 에서 1.2 Mbps 이하로 내려가고, 5 Mbps 로 풀리면 1.5 Mbps 넘게 오르나?
- 기준: 테스트 파일 5개(6건)가 통과하면 해당 질문 통과. 각 테스트는 측정값을 출력한다 (`--nocapture`).
- 이미 소스로 확인한 설계 영향 (결과 문서에 함께 적는다)
  - srflx 후보만 local 에 두면 연결 확인 요청을 받지 못한다 (`is-0.11.0/src/agent.rs:1444-1450`: Host·Relayed 후보 주소로 온 STUN 요청만 받음). UPnP 매핑을 쓰려면 같은 socket 의 host 후보를 local 에 함께 두고 SDP 에는 원하는 것만 보낸다. `external_candidate.rs` 두 번째 테스트가 이를 확인한다.
  - 선택된 경로는 event 가 아니라 `set_stats_interval` 의 `Event::PeerStats` 로만 보인다 (`stats.rs:85`, `lib.rs:2047-2056`). 경로 기록(spec 5.4)은 이것과 Transmit 주소로 만든다.
  - key frame 요청은 받는 쪽이 `rtc.writer(mid)` → `Writer::request_keyframe(None, KeyframeRequestKind::Pli)` 로 보내고, 보내는 쪽은 `Event::KeyframeRequest` 로 받는다 (`media/writer.rs:221-243`).
  - ICE restart 에서 `keep_local_candidates=false` 면 새 host 후보는 `accept_answer` 뒤에 넣고 trickle 로 보낸다 (`change/sdp.rs:734-751`).
- 파일
  - 만들기: `spikes/webrtc-str0m/Cargo.toml`, `spikes/webrtc-str0m/src/lib.rs`, `spikes/webrtc-str0m/tests/common/mod.rs`, `spikes/webrtc-str0m/tests/external_candidate.rs`, `spikes/webrtc-str0m/tests/ice_restart.rs`, `spikes/webrtc-str0m/tests/h264.rs`, `spikes/webrtc-str0m/tests/data_channel.rs`, `spikes/webrtc-str0m/tests/bwe.rs` (부록 D)
- 다른 task 와 주고받는 것: package 이름 `spike-webrtc-str0m` (Task 6 이 Windows 에서 같은 테스트를 실행)

- [ ] **1단계: 테스트 5개 작성** — 부록 D 의 `Cargo.toml`, `src/lib.rs`, `tests/*.rs` (common 제외).
- [ ] **2단계: 실패 확인** — `cd spikes/webrtc-str0m && cargo test --no-run`. 기대: `tests/common/mod.rs` 가 없어 `file not found for module common` 컴파일 오류.
- [ ] **3단계: harness 작성** — 부록 D 의 `tests/common/mod.rs` (시간을 흉내 내는 가짜 네트워크: 지연, 병목 token bucket, n번째 패킷 버리기, NAT 주소 바꾸기, 주소 막기).
- [ ] **4단계: 실행**

```bash
cd spikes/webrtc-str0m
cargo test -- --nocapture --test-threads=1 2>&1 | tee /tmp/str0m-spike.log
```

기대: 6 passed. 출력에서 ICE restart 복구 시간, 신뢰·비신뢰 수신 개수, 대역폭 추정 시간표를 결과 문서에 옮긴다.
실패하면: 먼저 harness 쪽인지 확인한다 (예: 손실 20% 로 SCTP 재전송이 60초를 넘으면 reliable 200건 assertion 이 시간 초과로 실패할 수 있음 → 로그의 재전송 시간으로 확인한 뒤에만 손실률을 낮춘다. 기준값을 바꾸면 결과 문서에 이유와 함께 적는다).

- [ ] **5단계: 결과 기록** — `## T4` 에 질문별 판정과 측정값.
- [ ] **6단계: commit** — `spikes/webrtc-str0m/` 파일 각각과 결과 문서, "test: str0m WebRTC spike 추가와 결과 기록".

---

## Task 5. WebRTC: webrtc-rs

- 실행 위치: 클라우드
- 질문: Task 4 와 같은 5개. 비교를 위해 같은 조건(병목 1→5 Mbps, 5번째마다 손실, 200건)으로 만든다.
- 기준: 테스트 파일 5개가 통과하면 해당 질문 통과.
- 이미 소스로 확인한 설계 영향
  - 비동기 `PeerConnection` 에는 local 후보 추가 API 가 없다 (`rtc` 의 `add_local_candidate` 는 driver 만 호출). 외부 후보는 SDP 문자열을 고쳐 넣는다. `with_nat_1to1_ips` 는 값을 저장만 하고 읽는 곳이 없다 (`setting_engine.rs:792-799`).
  - 보내는 쪽이 PLI 를 받으려면 `DeliverToApplication` 표시를 하는 interceptor 가 필요하다 (`examples/rtcp-processing`).
  - GCC 대역폭 추정은 `configure_congestion_control` 로 켜야 하고 (`interceptor_registry.rs:668`), 추정값은 직접 감싼 estimator 로 꺼낸다.
  - mDNS 기본값(QueryOnly)은 UDP 5353 bind 실패가 연결 생성 실패가 된다 (`driver.rs:443-445`). 테스트는 `Disabled` 로 둔다.
  - `H264Payloader` 는 SPS/PPS 를 다음 NAL 앞에 STAP-A 로 묶어 보내는데, 묶음이 MTU(1200) 를 넘으면 오류 없이 버린다 (`rtc-rtp` `codec/h264/mod.rs:104`). `h264.rs` 의 unit test 가 이 동작을 고정한다. 제품에서 쓰면 SPS/PPS 크기를 확인하는 코드가 필요하다.
  - remote track 은 첫 RTP packet 이 와야 생기고 event 와 media 가 다른 queue 를 탄다 (`rtc` `handler/interceptor.rs:643-649`). `h264.rs` 는 `on_track` 전까지 작은 warm-up frame 을 보낸다.
  - loopback 에서는 host 후보끼리 직접 닿으므로, 중계 socket(손실·병목)을 반드시 거치게 하려고 offerer 후보를 SDP 에서 모두 지우고 answerer 후보를 중계 주소로 바꾼다. answerer 는 들어오는 check 로 상대를 prflx 후보로 알게 된다. 이 방식이 실제로 연결되는지는 실행으로 확인한다.
- 파일
  - 만들기: `spikes/webrtc-rs/Cargo.toml`, `spikes/webrtc-rs/src/lib.rs`, `spikes/webrtc-rs/tests/common/mod.rs`, `spikes/webrtc-rs/tests/external_candidate.rs`, `spikes/webrtc-rs/tests/ice_restart.rs`, `spikes/webrtc-rs/tests/h264.rs`, `spikes/webrtc-rs/tests/data_channel.rs`, `spikes/webrtc-rs/tests/bwe.rs` (부록 E)
- 다른 task 와 주고받는 것: package 이름 `spike-webrtc-rs` (Task 6)

- [ ] **1단계: 테스트 5개 작성** — 부록 E (common 제외).
- [ ] **2단계: 실패 확인** — `cd spikes/webrtc-rs && cargo test --no-run`. 기대: `common` 모듈 없음 컴파일 오류.
- [ ] **3단계: harness 작성** — 부록 E 의 `tests/common/mod.rs` (실제 loopback UDP socket, 손실·병목을 넣는 UDP 중계 task).
- [ ] **4단계: 실행**

```bash
cd spikes/webrtc-rs
cargo test -- --nocapture --test-threads=1 2>&1 | tee /tmp/webrtc-rs-spike.log
```

기대: 전부 통과. 실제 socket 과 벽시계를 쓰므로 Task 4 보다 오래 걸린다 (측정해 기록).

- [ ] **5단계: 결과 기록** — `## T5`.
- [ ] **6단계: commit** — "test: webrtc-rs WebRTC spike 추가와 결과 기록".

---

## Task 6. WebRTC: Windows 빌드와 IPv6

- 실행 위치: GitHub Actions + 사람 Windows PC (IPv6 한 가지)
- 질문
  1. 두 라이브러리가 `x86_64-pc-windows-msvc` 로 빌드되나? (aws-lc-sys, ring 의 C 코드 빌드가 MSVC 에서 되나)
  2. Task 4, 5 테스트가 Windows 에서도 통과하나? (webrtc-rs 는 Windows socket·interface 열거 코드를 탄다)
  3. Windows 에서 IPv6 host 후보로 실제 UDP 연결이 되나? `::1` 은 Actions 에서, 실제 공인 IPv6 주소는 사람 PC 에서.
- 기준
  - 통과: 두 package 의 Windows 빌드 성공, 테스트 결과가 Linux 와 같음, `ipv6-pair ::1` 이 `RESULT ... ok=true`.
  - 사람 PC: `ipconfig` 에 전역 IPv6 주소(`2` 또는 `3` 으로 시작)가 있으면 그 주소로 `ok=true`. 주소가 없으면 "IPv6 없음" 으로 기록하고 통과·실패 판정에서 뺀다.
- 파일
  - 만들기: `spikes/ipv6-pair/Cargo.toml`, `spikes/ipv6-pair/src/main.rs` (부록 F)
  - 고치기: `.github/workflows/windows-spikes.yml` (부록 A 의 `# Task 6` step)
- 다른 task 와 주고받는 것: artifact `win-spikes` 안의 `ipv6-pair.exe`
- 확인할 가정: webrtc-rs 테스트는 중계 socket 을 `127.0.0.2` 에 bind 한다. Linux 는 127/8 전체가 loopback 이다. Windows 에서도 bind 되는지는 이 task 의 Actions 실행이 확인한다. 안 되면 오류 줄로 확인한 뒤 중계 주소를 `127.0.0.1` 의 다른 port 로 바꾸고, 바꾼 사실을 결과 문서에 적는다.

- [ ] **1단계: 작성** — 부록 F, 부록 A 의 Task 6 step.
- [ ] **2단계: 클라우드 확인** — `cd spikes/ipv6-pair && cargo run -- ::1`. 기대: `RESULT ip=::1 ok=true`.
- [ ] **3단계: commit, push, Actions 로그 확인** — 기대: 두 package 의 `test result: ok` 와 `RESULT ip=::1 ok=true`. 실패하면 로그의 첫 오류 줄부터 원인을 찾는다.
- [ ] **4단계: 사람 실행 (Windows PC)**

```powershell
ipconfig | Select-String "IPv6"
C:\spike\ipv6-pair.exe <위에서 본 2 또는 3 으로 시작하는 주소>
```

Windows 방화벽 창이 뜨면 "허용"을 누르지 않고 "취소"를 눌러도 같은 PC 안 통신은 된다. 출력 전체를 대화창에 붙여 넣는다.

- [ ] **5단계: 결과 기록과 commit** — `## T6`, "test: WebRTC Windows 빌드·IPv6 spike 결과 기록".

---

## Task 7. WebRTC 라이브러리 결정

- 실행 위치: 클라우드 (코드 없음) + 사람 확인
- 입력: `## T4`, `## T5`, `## T6`
- 규칙 (이 계획 "새로 정한 것" 3번): 질문 5개 통과 수 → Windows 빌드·테스트 → 동점이면 `str0m`.
- [ ] **1단계:** 결과 문서 `## T7` 에 비교표(질문별 통과, 측정값, Windows, 발견한 우회 필요 사항)와 선택안을 쓴다.
- [ ] **2단계:** 사람에게 선택안을 보여 주고 승인을 받는다. 승인 전에는 `crates/transport` 계획을 쓰지 않는다.
- [ ] **3단계:** 승인되면 진행 기록 결정 기록에 추가하고, spec 3.3 표의 WebRTC 라이브러리 행을 확정 값으로 고친다 (spec 과 진행 기록 동시 수정, CLAUDE.md 규칙).
- [ ] **4단계: commit** — "docs: WebRTC 라이브러리 결정 기록".

---

## Task 8. Cloudflare Workers 로컬 테스트

- 실행 위치: 클라우드
- 질문: Cloudflare 계정 없이 Durable Object + WebSocket(Hibernation API) 을 로컬 테스트할 수 있나?
- 기준: 통과 = 테스트 4건 통과 (같은 방 두 socket 사이 전달, 다른 방으로 새지 않음, WebSocket 아닌 요청 426, DO 안의 socket 수 2) 와 `tsc --noEmit` 통과, 로그인 없이.
- 파일
  - 만들기: `spikes/signaling/package.json`, `spikes/signaling/package-lock.json`(설치로 생성), `spikes/signaling/wrangler.jsonc`, `spikes/signaling/vitest.config.ts`, `spikes/signaling/tsconfig.json`, `spikes/signaling/src/index.ts`, `spikes/signaling/test/room.test.ts`, `spikes/signaling/worker-configuration.d.ts`(생성) (부록 G)
- 다른 task 와 주고받는 것: 없음. 결과는 이후 `signaling/` 계획의 입력.
- 주의: Node 22 에 딸린 npm 10.9.7 은 이 의존성 조합에서 `Cannot read properties of null (reading 'edgesOut')` 로 설치에 실패한다 (npm arborist 의 peer 처리 문제). `npx -y npm@11` 로 설치한다.

- [ ] **1단계: 테스트 먼저** — 부록 G 의 `package.json`, `wrangler.jsonc`, `vitest.config.ts`, `tsconfig.json`, `test/room.test.ts` 작성.
- [ ] **2단계: 실패 확인**

```bash
cd spikes/signaling
npx -y npm@11 install
npx vitest run
```

기대: `src/index.ts` 가 없어 실패.

- [ ] **3단계: 최소 구현** — 부록 G 의 `src/index.ts`. 그다음 `npx wrangler types --include-runtime=false` 로 `worker-configuration.d.ts` 생성.
- [ ] **4단계: 통과 확인**

```bash
npx vitest run --reporter=verbose
npx tsc --noEmit
```

기대: `Tests 4 passed (4)`, tsc 출력 없음.

- [ ] **5단계: 결과 기록과 commit** — `## T8` 에 판정, 설치한 버전, npm 11 필요 사실, `SELF`·`env` (cloudflare:test) deprecation 과 대체(`cloudflare:workers` 의 `exports`, `env`). 파일을 하나씩 add, "test: Cloudflare Workers 로컬 테스트 spike 추가".

---

## Task 9. SYSTEM 계정 DPAPI

- 실행 위치: GitHub Actions (안 되면 사람 PC 로 대체, 아래 5단계)
- 질문: LocalSystem 이 `CryptProtectData`(entropy 없음, `CRYPTPROTECT_LOCAL_MACHINE` 없음)로 암호화한 데이터를 (a) SYSTEM 은 풀 수 있고 (b) 관리자 계정(다른 계정)은 풀 수 없나?
- 기준: 통과 = SYSTEM 으로 암호화 → SYSTEM 으로 복호화 `plaintext=spike-secret`, 관리자 계정으로 복호화는 오류. 둘 중 하나라도 다르면 실패이고 spec 4절 저장 방식을 다시 정해야 한다.
- 파일
  - 고치기: `.github/workflows/windows-spikes.yml` (부록 A 의 `# Task 9` step). `dpapi_probe` 는 Task 1 에서 이미 있음.
- 다른 task 와 주고받는 것: 없음.
- SYSTEM 으로 실행하는 방법: PsExec `-s`(SYSTEM 계정) `-accepteula`. runner 는 관리자 권한으로 돌므로 PsExec 를 쓸 수 있다고 가정하고, 이 가정 자체를 이 task 가 확인한다.

- [ ] **1단계: 실패 확인** — 부록 A 의 Task 9 step 을 넣기 전 로그에 `RESULT dpapi-system` 줄이 없음.
- [ ] **2단계: step 추가, commit, push.**
- [ ] **3단계: 로그 확인** — 기대 3줄:
  - `user=SYSTEM ...` 과 `protected ...` (SYSTEM 암호화)
  - `RESULT dpapi-system-decrypt ok=true` (SYSTEM 복호화, `plaintext=spike-secret`)
  - `RESULT dpapi-admin-decrypt ok=true` (관리자 복호화가 **실패한 것**이 기대값이라 ok=true)
- [ ] **4단계: 결과 기록과 commit** — `## T9`.
- [ ] **5단계 (Actions 에서 PsExec 실행이 막힌 경우에만): 사람 PC**

관리자 PowerShell 에서:

```powershell
cd C:\spike
"spike-secret" | Out-File -Encoding ascii -NoNewline plain.txt
.\PsExec64.exe -accepteula -s C:\spike\dpapi_probe.exe protect C:\spike\plain.txt C:\spike\sys.bin
.\PsExec64.exe -accepteula -s C:\spike\dpapi_probe.exe unprotect C:\spike\sys.bin
C:\spike\dpapi_probe.exe unprotect C:\spike\sys.bin
```

기대: 두 번째 명령은 `user=SYSTEM` 과 `plaintext=spike-secret`, 세 번째(내 계정)는 오류. 출력 전체를 붙여 넣는다.

---

## Task 10. SendInput 절대 좌표

- 실행 위치: 사람 Windows PC
- 질문: physical pixel virtual desktop 좌표를 0..65535 로 바꾸는 두 공식 중 어느 것이 모든 모니터의 모서리·가운데에서 커서를 정확한 픽셀에 놓나?
  - 공식 A: `((x - vx) * 65535) / (vw - 1)`
  - 공식 B: `((x - vx) * 65536 + vw / 2) / vw`
- 기준: 통과 = 한 공식이 불일치 0건. 두 공식 다 불일치가 있으면 실패로 기록하고 불일치 CSV 를 분석한다 (주입 후 되읽어 보정하는 방식 검토). 모니터가 1개거나 배율이 모두 같으면 그 조건을 적고 "부분" 판정.
- 파일: 없음 (Task 1 의 `sendinput_probe.exe`)
- 사람 실행

```powershell
cd C:\spike
.\sendinput_probe.exe > sendinput.csv
Get-Content sendinput.csv | Select-Object -First 3
```

실행하는 몇 초 동안 마우스가 여러 모니터를 돌아다닌다. 마우스를 건드리지 않는다. 창에 찍힌 마지막 줄(`points=... formulaA_mismatches=... formulaB_mismatches=...`)과 `virtual_desktop`, `monitor[...]` 줄, `sendinput.csv` 전체를 붙여 넣는다. 모니터별 배율(설정 → 디스플레이)도 함께 적는다.

- [ ] **1단계: 사람 실행과 결과 받기.**
- [ ] **2단계: 결과 기록과 commit** — `## T10` 에 모니터 구성, 공식별 불일치 수, 선택한 공식. "docs: SendInput 좌표 spike 결과 기록".

---

## Task 11. MF 하드웨어 인코더 (SYSTEM agent)

- 실행 위치: 사람 Windows PC (GPU 필요)
- 질문: 사용자 세션에서 도는 SYSTEM 프로세스(spec 의 host-agent 와 같은 조건)가 하드웨어 H.264 MFT 를 만들고, D3D11 NV12 texture 를 입력해 인코딩할 수 있나? 안 되면 소프트웨어 MFT 는 되나?
- 기준
  - 통과: SYSTEM + 사용자 세션 번호로 `RESULT mode=hardware ... frames_in=30 frames_out>=25 bytes_out>0`.
  - 부분: 하드웨어는 실패, 소프트웨어는 `frames_out>=25`. spec 6.2 의 "하드웨어 우선"이 이 PC 에서는 대체로만 동작함을 기록.
  - 실패: 둘 다 실패. HRESULT 를 기록하고 원인을 찾는다.
  - 비교용으로 내 계정 실행 결과도 받는다 (SYSTEM 에서만 실패하는지 가리기 위해).
- 파일: 없음 (Task 1 의 `mf_probe.exe`)
- 사람 실행 (관리자 PowerShell)

```powershell
cd C:\spike
query session
# 출력에서 상태가 Active 인 줄의 ID 숫자를 아래 <세션> 에 넣는다
.\mf_probe.exe
.\mf_probe.exe --software
.\PsExec64.exe -accepteula -s -i <세션> C:\spike\mf_probe.exe > mf-system.txt 2>&1
.\PsExec64.exe -accepteula -s -i <세션> C:\spike\mf_probe.exe --software > mf-system-sw.txt 2>&1
Get-Content mf-system.txt, mf-system-sw.txt
```

`-i` 로 띄운 프로세스 출력이 파일에 비어 있으면 PsExec 가 출력을 넘기지 못한 것이다. 그때는 `-i` 없이 한 번 더 실행해 결과를 함께 보낸다 (session 0 결과로 표시해 기록). 모든 출력을 붙여 넣는다.

- [ ] **1단계: 사람 실행과 결과 받기.**
- [ ] **2단계: 결과 기록과 commit** — `## T11` 에 GPU, 인코더 이름, 계정·세션, frames/bytes, 첫 출력까지 시간(ms), 실패 HRESULT.

---

## Task 12. egui

- 실행 위치: GitHub Actions (빌드) + 사람 Windows PC (측정)
- 질문
  1. 1920x1080 영상 texture 를 30fps 로 갱신할 때 한 프레임 처리 시간은? `set` 과 `set_partial` 중 어느 쪽이 빠른가?
  2. Windows 기본 한국어 IME 로 한글 입력(조합 표시, 확정, 백스페이스)이 정상인가?
  3. 배율이 다른 모니터로 창을 옮기면 `pixels_per_point` 가 모니터 배율을 따라가고 글자가 흐려지지 않나?
- 기준
  - 1: 통과 = 30fps 유지(`paints/s` 29 이상), `set_partial` 모드의 `cpu_usage` p95 16ms 이하 (60Hz 한 프레임. spec 10절 처리 지연 100ms 목표의 여유를 남기기 위한 설계 제안값).
  - 2: 통과 = 아래 문장을 두 입력 칸에 입력했을 때 화면 글자와 입력한 글자가 같고, 조합 중 글자가 보이며, 조합 중 백스페이스가 자모 하나만 지운다. 문장: `원격 지원 테스트 한글 입력 확인`
  - 3: 통과 = 모니터 이동 후 `pixels_per_point` 가 그 모니터 배율(예: 150% → 1.5)과 같고 글자가 선명함. 모니터가 1개면 설정에서 배율을 바꿔 확인.
- 파일
  - 만들기: `spikes/egui-probe/Cargo.toml`, `spikes/egui-probe/src/main.rs` (부록 H)
  - 고치기: `.github/workflows/windows-spikes.yml` (부록 A 의 `# Task 12` step)
- 다른 task 와 주고받는 것: artifact 안 `egui_probe.exe`

- [ ] **1단계: 작성과 클라우드 확인** — `cd spikes/egui-probe && cargo build --release` (Linux 빌드 성공만 확인. 이 컨테이너는 `libxkbcommon-x11` 이 없어 실행 불가).
- [ ] **2단계: commit, push, Actions 에서 exe 생성 확인.**
- [ ] **3단계: 사람 실행**

```powershell
C:\spike\egui_probe.exe
```

1. 창을 켠 채 30초 기다린 뒤 오른쪽 패널의 숫자 줄(`paints/s` 부터 `cpu_usage` 까지)을 사진 찍거나 옮겨 적는다.
2. "upload with set_partial" 체크를 끄고 30초 뒤 같은 줄을 다시 적는다.
3. single-line, multi-line 칸에 위 문장을 입력한다. 조합 중에 백스페이스를 한 번 눌러 본다. 아래 "last 20 Ime/Text events" 목록을 적는다.
4. 창을 다른 모니터로 옮기고 `pixels_per_point` 줄을 적는다.

- [ ] **4단계: 결과 기록과 commit** — `## T12`.

---

## Task 13. WiX MSI

- 실행 위치: GitHub Actions (빌드, 설치, 검사, 제거)
- 질문
  1. WiX 7 로 LocalSystem 자동 시작 서비스 + 실패 시 재시작 + 프로그램 한정 방화벽 규칙 + SYSTEM 전용 ProgramData 폴더를 가진 MSI 를 만들 수 있나?
  2. 설치 후 실제 상태가 선언과 같나? (서비스 계정, 시작 유형, 실패 동작, 방화벽 규칙, 폴더 ACL)
  3. 서비스가 비정상 종료하면 재시작되나?
  4. 제거 후 서비스, 파일, 방화벽 규칙, ProgramData 폴더(서비스가 만든 로그 포함)가 모두 사라지나?
- 기준: 통과 = 부록 I 의 `check.ps1` 이 `RESULT wix ok=true` 를 출력 (각 항목 `CHECK <이름> ok=<bool>` 줄로 개별 판정).
- 이 task 가 확인할 가정 (소스로 확인 못 한 것)
  - `PermissionEx Sddl="D:P(A;OICI;FA;;;SY)"` 가 상속 ACL 을 대체하는지 (`icacls` 로 확인)
  - `util:RemoveFolderEx` 에 `%ProgramData%` 가 든 property 가 CostInitialize 전에 경로로 풀리는지 (xsd 설명: property 값의 `%환경 변수%` 는 펼쳐진다, Directory id 는 쓸 수 없다)
  - `Account="LocalSystem"` 표기가 맞는지 (`sc qc` 로 확인)
- 파일
  - 만들기: `spikes/wix/service/Cargo.toml`, `spikes/wix/service/src/main.rs`, `spikes/wix/Spike.wxs`, `spikes/wix/check.ps1` (부록 I)
  - 고치기: `.github/workflows/windows-spikes.yml` (부록 A 의 `# Task 13` step)
- 다른 task 와 주고받는 것: 서비스 이름 `SimpleRemoteSpike`, 폴더 `C:\Program Files\SimpleRemoteSpike`, `C:\ProgramData\SimpleRemoteSpike`
- 이용 조건: WiX 7 은 OSMF EULA 를 수락해야 빌드한다 (`-acceptEula wix7`). 연 매출 US$10,000 미만이거나 수익 활동이 아니면 유지 보수 요금 면제 (OSMFEULA.txt 1절). 이 프로젝트는 개인 비영리라 면제 대상이다.

- [ ] **1단계: 실패 확인** — Task 13 step 을 넣기 전 로그에 `RESULT wix` 줄이 없음.
- [ ] **2단계: 작성과 클라우드 컴파일 확인**

```bash
cd spikes/wix/service
cargo check --target x86_64-pc-windows-msvc
cargo check
```

기대: 둘 다 `Finished` (Linux 는 `windows only` 를 출력하는 빈 main).

- [ ] **3단계: commit, push, 로그 확인** — 기대: `CHECK` 줄 전부 `ok=true`, 마지막 `RESULT wix ok=true`. `ok=false` 인 항목은 원인을 로그와 `msiexec` 로그(`install.log`, `uninstall.log`, artifact `wix-logs`)로 확인한다.
- [ ] **4단계: 결과 기록과 commit** — `## T13` 에 항목별 판정, 확인 못 한 가정의 결과, 레지스트리 정책 값 복원(`SoftwareSASGeneration`)은 WiX 선언으로 안 되고 custom action 이 필요하다는 소스 확인 결과(`RegistryValue` 의 Action 은 `append|prepend|write` 뿐).

---

## Task 14. 종합과 문서 반영

- 실행 위치: 클라우드 + 사람 확인
- [ ] **1단계:** 결과 문서 `## 종합` 에 spike 별 판정 표와 spec 에 반영할 변경 목록을 쓴다. 이미 예상되는 후보:
  - spec 3.3 WebRTC 라이브러리 확정 (Task 7)
  - spec 5.2, 5.7 의 PAKE 라이브러리와 "RFC 9382 테스트 값" 문구 (Task 2 결과로)
  - spec 4절에 재접속용 X25519 정적 key (Task 3)
  - spec 5.4 에 "UPnP 매핑 후보는 같은 socket 의 host 후보와 함께 둔다" (Task 4)
  - spec 6.3 의 좌표 변환 공식 (Task 10)
  - spec 7.1 의 정책 값 복원 방식 (custom action), ProgramData 삭제 방식 (Task 13)
- [ ] **2단계:** 사람에게 변경 목록 승인을 받는다.
- [ ] **3단계:** 승인된 변경을 spec 과 진행 기록 결정 기록에 함께 반영하고, 진행 기록 "다음 할 일"을 다음 계획(무엇을 먼저 쓸지)으로 갱신한다.
- [ ] **4단계: commit** — "docs: Phase 0 spike 결과를 spec 과 진행 기록에 반영".

---

## spec 덮임 점검

spec 11절 표의 행과 이 계획의 task:

| spec 11절 행 | 확인할 내용 | task |
|---|---|---|
| WebRTC 라이브러리 | 외부 후보 추가 | 4, 5 (`external_candidate`) |
| | ICE restart | 4, 5 (`ice_restart`) |
| | H.264 RTP packetization | 4, 5 (`h264`) |
| | data channel 신뢰·비신뢰 | 4, 5 (`data_channel`) |
| | 대역폭 추정 | 4, 5 (`bwe`) |
| | Windows IPv6 후보 동작 | 6 |
| UI (egui) | 1080p30 텍스처 갱신 지연 | 12 (질문 1) |
| | 한글 IME | 12 (질문 2) |
| | Per-Monitor v2 DPI | 12 (질문 3) |
| PAKE | `spake2` 와 `opaque-ke` 비교, RFC 9382 테스트 값 | 2 |
| 재접속 key 합의 | Noise KK crate 선택 | 3 |
| SYSTEM 계정 DPAPI | 다른 계정이 풀 수 없는지 | 9 |
| MF 인코더 | SYSTEM agent(사용자 세션)에서 하드웨어 MFT, D3D11 texture 입력 | 11 |
| 입력 좌표 | `SendInput` 반올림 되읽기 | 10 |
| WiX Toolset | 버전과 이용 조건 | 13 (이용 조건 절, 버전 7.0.0) |
| | 서비스·방화벽·정책 복원 구성 | 13 (정책 복원은 custom action 필요 확인) |
| Cloudflare Workers 로컬 테스트 | DO·WebSocket 테스트 방법 | 8 |

진행 기록의 "Windows spike 빌드 방식(Windows PC 직접 / 교차 빌드)은 Phase 0 계획에서 정한다" → 이 계획 "새로 정한 것" 1번과 Task 1.

빈칸 점검: 이 문서에서 "TBD", "나중에", "적절히" 를 검색해 0건임을 확인했다.

---

# 부록

## 부록 A. `.github/workflows/windows-spikes.yml` (최종 형태)

각 step 위의 `# Task N` 주석이 그 step 을 넣는 task 다. Task 1 에서는 `# Task 1` step 과 `upload exes` step(경로 중 `win-probes` 줄만)을 넣고, 이후 task 가 자기 step 과 경로 줄을 더한다.

`.github/workflows/windows-spikes.yml`:

```yaml
# Spike only (throwaway): Windows build and automated checks for SP1 Phase 0.
# Plan: docs/superpowers/plans/2026-09-23-sp1-phase0-spikes.md
name: windows-spikes

on:
  push:
    paths:
      - 'spikes/**'
      - '.github/workflows/windows-spikes.yml'
  workflow_dispatch:

permissions:
  contents: read

jobs:
  build-test:
    runs-on: windows-2025
    defaults:
      run:
        shell: pwsh
    steps:
      # Task 1
      - uses: actions/checkout@v7

      # Task 1
      - name: toolchain
        working-directory: spikes
        run: |
          rustup toolchain install 1.95 --profile minimal
          rustup show active-toolchain
          rustc --version
          cargo --version

      # Task 1
      - name: build win-probes
        working-directory: spikes/win-probes
        run: cargo build --release --bins

      # Task 1
      - name: dpapi roundtrip (runner account)
        working-directory: spikes/win-probes/target/release
        run: |
          Set-Content -Path plain.txt -Value 'spike-secret' -NoNewline -Encoding ascii
          .\dpapi_probe.exe protect plain.txt user.bin
          $out = .\dpapi_probe.exe unprotect user.bin | Out-String
          Write-Output $out
          $ok = $out -match 'plaintext=spike-secret'
          Write-Output "dpapi roundtrip RESULT ok=$($ok.ToString().ToLower()) user=$env:USERNAME"
          if (-not $ok) { exit 1 }

      # Task 1 (informational: hosted runners have no GPU)
      - name: mf_probe (informational)
        continue-on-error: true
        working-directory: spikes/win-probes/target/release
        run: |
          .\mf_probe.exe
          .\mf_probe.exe --software

      # Task 6
      - name: webrtc str0m tests
        working-directory: spikes/webrtc-str0m
        run: cargo test -- --nocapture --test-threads=1

      # Task 6
      - name: webrtc-rs tests
        working-directory: spikes/webrtc-rs
        run: cargo test -- --nocapture --test-threads=1

      # Task 6
      - name: ipv6 pair on ::1
        working-directory: spikes/ipv6-pair
        run: |
          cargo build --release
          .\target\release\ipv6-pair.exe ::1

      # Task 9
      - name: fetch PsExec
        run: |
          Invoke-WebRequest -Uri https://download.sysinternals.com/files/PSTools.zip -OutFile $env:RUNNER_TEMP\PSTools.zip
          Expand-Archive $env:RUNNER_TEMP\PSTools.zip -DestinationPath $env:RUNNER_TEMP\pstools
          "PSEXEC=$env:RUNNER_TEMP\pstools\PsExec64.exe" | Out-File -FilePath $env:GITHUB_ENV -Append -Encoding utf8

      # Task 9
      - name: dpapi as SYSTEM
        working-directory: spikes/win-probes/target/release
        run: |
          $probe = (Resolve-Path .\dpapi_probe.exe).Path
          $plain = (Resolve-Path .\plain.txt).Path
          $blob = Join-Path (Get-Location) 'system.bin'
          & $env:PSEXEC -accepteula -nobanner -s $probe protect $plain $blob 2>&1 | Write-Output
          $sys = & $env:PSEXEC -accepteula -nobanner -s $probe unprotect $blob 2>&1 | Out-String
          Write-Output $sys
          $sysOk = ($sys -match 'user=SYSTEM') -and ($sys -match 'plaintext=spike-secret')
          Write-Output "RESULT dpapi-system-decrypt ok=$($sysOk.ToString().ToLower())"
          $admin = & $probe unprotect $blob 2>&1 | Out-String
          Write-Output $admin
          # Expected: the admin (runner) account can NOT decrypt the SYSTEM blob.
          $adminOk = -not ($admin -match 'plaintext=spike-secret')
          Write-Output "RESULT dpapi-admin-decrypt ok=$($adminOk.ToString().ToLower())"
          if (-not ($sysOk -and $adminOk)) { exit 1 }

      # Task 12
      - name: build egui-probe
        working-directory: spikes/egui-probe
        run: cargo build --release

      # Task 13
      - name: wix build, install, check, uninstall
        working-directory: spikes/wix
        run: |
          dotnet tool install --global wix --version 7.0.0
          $wix = "$env:USERPROFILE\.dotnet\tools\wix.exe"
          & $wix --version
          & $wix extension add -g WixToolset.Util.wixext/7.0.0 -acceptEula wix7
          & $wix extension add -g WixToolset.Firewall.wixext/7.0.0 -acceptEula wix7
          cargo build --release --manifest-path service\Cargo.toml
          Copy-Item service\target\release\spike-service.exe .
          & $wix build Spike.wxs -arch x64 -ext WixToolset.Util.wixext -ext WixToolset.Firewall.wixext -acceptEula wix7 -o SimpleRemoteSpike.msi
          if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
          .\check.ps1 -Msi (Resolve-Path .\SimpleRemoteSpike.msi).Path -PsExec $env:PSEXEC

      # Task 13
      - name: upload wix logs
        if: always()
        uses: actions/upload-artifact@v7
        with:
          name: wix-logs
          path: spikes/wix/*.log
          if-no-files-found: ignore
          retention-days: 7

      # Task 1 (Task 6, 12 add their exe paths below)
      - name: upload exes
        if: always()
        uses: actions/upload-artifact@v7
        with:
          name: win-spikes
          path: |
            spikes/win-probes/target/release/*.exe
            spikes/ipv6-pair/target/release/ipv6-pair.exe
            spikes/egui-probe/target/release/egui_probe.exe
          if-no-files-found: warn
          retention-days: 14
```

---

## 부록 B. `spikes/win-probes`

`spikes/win-probes/Cargo.toml`:

```toml
[package]
name = "spike-win-probes"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "dpapi_probe"
path = "src/bin/dpapi_probe.rs"

[[bin]]
name = "sendinput_probe"
path = "src/bin/sendinput_probe.rs"

[[bin]]
name = "mf_probe"
path = "src/bin/mf_probe.rs"

[dependencies.windows]
version = "=0.62.2"
features = [
  "Win32_Foundation",
  "Win32_Security_Cryptography",
  "Win32_System_Com",
  "Win32_System_Threading",
  "Win32_System_RemoteDesktop",
  "Win32_System_WindowsProgramming",
  "Win32_UI_Input_KeyboardAndMouse",
  "Win32_UI_WindowsAndMessaging",
  "Win32_UI_HiDpi",
  "Win32_Graphics_Gdi",
  "Win32_Graphics_Direct3D",
  "Win32_Graphics_Direct3D10",
  "Win32_Graphics_Direct3D11",
  "Win32_Graphics_Dxgi",
  "Win32_Graphics_Dxgi_Common",
  "Win32_Media_MediaFoundation",
]
```

`spikes/win-probes/src/bin/dpapi_probe.rs`:

```rust
// Spike only (throwaway).
// dpapi_probe: DPAPI round-trip using CryptProtectData/CryptUnprotectData (no entropy, UI_FORBIDDEN).
// Usage: dpapi_probe protect <infile> <outfile>
//        dpapi_probe unprotect <infile>
// Prints current user name and session id first.

use std::fs;

use windows::core::{PWSTR, Result};
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::System::WindowsProgramming::GetUserNameW;

fn current_user() -> String {
    let mut buf = [0u16; 256];
    let mut len = buf.len() as u32;
    unsafe {
        match GetUserNameW(Some(PWSTR(buf.as_mut_ptr())), &mut len) {
            Ok(()) => {
                // len includes the terminating null.
                let n = (len as usize).saturating_sub(1);
                String::from_utf16_lossy(&buf[..n])
            }
            Err(e) => format!("<GetUserNameW failed: {e}>"),
        }
    }
}

fn current_session() -> u32 {
    let mut sid = 0u32;
    unsafe {
        let pid = GetCurrentProcessId();
        match ProcessIdToSessionId(pid, &mut sid) {
            Ok(()) => sid,
            Err(_) => u32::MAX,
        }
    }
}

/// Copy the DPAPI output blob into a Vec and free the DPAPI-allocated buffer.
unsafe fn take_blob(blob: &CRYPT_INTEGER_BLOB) -> Vec<u8> {
    let out = if blob.cbData == 0 || blob.pbData.is_null() {
        Vec::new()
    } else {
        std::slice::from_raw_parts(blob.pbData, blob.cbData as usize).to_vec()
    };
    if !blob.pbData.is_null() {
        let _ = LocalFree(Some(HLOCAL(blob.pbData as *mut _)));
    }
    out
}

fn protect(input: &[u8]) -> Result<Vec<u8>> {
    let data_in = CRYPT_INTEGER_BLOB {
        cbData: input.len() as u32,
        pbData: input.as_ptr() as *mut u8,
    };
    let mut data_out = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptProtectData(
            &data_in,
            None,                          // szdatadescr
            None,                          // poptionalentropy (no entropy)
            None,                          // pvreserved
            None,                          // ppromptstruct
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut data_out,
        )?;
        Ok(take_blob(&data_out))
    }
}

fn unprotect(input: &[u8]) -> Result<Vec<u8>> {
    let data_in = CRYPT_INTEGER_BLOB {
        cbData: input.len() as u32,
        pbData: input.as_ptr() as *mut u8,
    };
    let mut data_out = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptUnprotectData(
            &data_in,
            None,                          // ppszdatadescr
            None,                          // poptionalentropy
            None,                          // pvreserved
            None,                          // ppromptstruct
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut data_out,
        )?;
        Ok(take_blob(&data_out))
    }
}

fn main() -> Result<()> {
    println!("user={} session={}", current_user(), current_session());

    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("protect") => {
            let infile = &args[2];
            let outfile = &args[3];
            let input = fs::read(infile).expect("read infile");
            let out = protect(&input)?;
            fs::write(outfile, &out).expect("write outfile");
            println!("protected {} bytes -> {} bytes", input.len(), out.len());
        }
        Some("unprotect") => {
            let infile = &args[2];
            let input = fs::read(infile).expect("read infile");
            let out = unprotect(&input)?;
            println!("unprotected {} bytes -> {} bytes", input.len(), out.len());
            // Print recovered plaintext if it is valid UTF-8.
            if let Ok(s) = std::str::from_utf8(&out) {
                println!("plaintext={s}");
            }
        }
        _ => {
            eprintln!("usage: dpapi_probe protect <infile> <outfile> | unprotect <infile>");
        }
    }
    Ok(())
}
```

`spikes/win-probes/src/bin/sendinput_probe.rs`:

```rust
// Spike only (throwaway).
// sendinput_probe: verify absolute mouse positioning across a multi-monitor virtual desktop.
//
// Sets Per-Monitor-Aware-v2 DPI awareness, enumerates monitors (rects in physical px),
// then for a grid of target points per monitor converts virtual-desktop px -> 0..65535 with
// two formulas and issues an absolute SendInput move (VIRTUALDESK). After each move it reads
// GetCursorPos and prints a CSV row plus a mismatch summary.

use windows::core::BOOL;
use windows::Win32::Foundation::{LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_MOVE,
    MOUSEEVENTF_VIRTUALDESK, MOUSEINPUT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
    SM_YVIRTUALSCREEN,
};

unsafe extern "system" fn enum_proc(
    hmon: HMONITOR,
    _hdc: HDC,
    _rc: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let out = &mut *(lparam.0 as *mut Vec<RECT>);
    let mut mi = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if GetMonitorInfoW(hmon, &mut mi).as_bool() {
        out.push(mi.rcMonitor);
    }
    BOOL(1) // continue enumeration
}

fn monitors() -> Vec<RECT> {
    let mut rects: Vec<RECT> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(enum_proc),
            LPARAM(&mut rects as *mut _ as isize),
        );
    }
    rects
}

// Formula A: ((x - vx) * 65535) / (vw - 1)
fn norm_a(x: i32, v0: i32, span: i32) -> i32 {
    if span <= 1 {
        return 0;
    }
    (((x - v0) as i64 * 65535) / (span as i64 - 1)) as i32
}

// Formula B (rounding): ((x - vx) * 65536 + vw/2) / vw
fn norm_b(x: i32, v0: i32, span: i32) -> i32 {
    if span <= 0 {
        return 0;
    }
    (((x - v0) as i64 * 65536 + (span as i64) / 2) / span as i64) as i32
}

fn send_abs(nx: i32, ny: i32) {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: nx,
                dy: ny,
                mouseData: 0,
                dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    unsafe {
        SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }
}

fn cursor() -> (i32, i32) {
    let mut p = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut p);
    }
    (p.x, p.y)
}

fn targets_for(rc: &RECT) -> Vec<(i32, i32)> {
    let (l, t, r, b) = (rc.left, rc.top, rc.right - 1, rc.bottom - 1);
    let cx = (rc.left + rc.right) / 2;
    let cy = (rc.top + rc.bottom) / 2;
    vec![
        (l, t),
        (r, t),
        (l, b),
        (r, b),
        (cx, cy),
        (l + 1, t + 1),
        (r - 1, b - 1),
        (cx, t),
        (cx, b),
        (l, cy),
        (r, cy),
    ]
}

fn main() {
    unsafe {
        // Per-monitor v2 so all coordinates are physical pixels.
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    let vx = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let vy = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
    let vw = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
    let vh = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
    eprintln!("virtual_desktop origin=({vx},{vy}) size=({vw}x{vh})");

    let mons = monitors();
    for (i, m) in mons.iter().enumerate() {
        eprintln!(
            "monitor[{i}] rect=({},{})-({},{})",
            m.left, m.top, m.right, m.bottom
        );
    }

    println!("target_x,target_y,formula,nx,ny,got_x,got_y,dx,dy");
    let mut mismatch_a = 0u32;
    let mut mismatch_b = 0u32;
    let mut total = 0u32;

    for m in &mons {
        for (tx, ty) in targets_for(m) {
            for (name, nx, ny) in [
                ("A", norm_a(tx, vx, vw), norm_a(ty, vy, vh)),
                ("B", norm_b(tx, vx, vw), norm_b(ty, vy, vh)),
            ] {
                send_abs(nx, ny);
                let (gx, gy) = cursor();
                let (dx, dy) = (gx - tx, gy - ty);
                println!("{tx},{ty},{name},{nx},{ny},{gx},{gy},{dx},{dy}");
                if dx != 0 || dy != 0 {
                    match name {
                        "A" => mismatch_a += 1,
                        _ => mismatch_b += 1,
                    }
                }
            }
            total += 1;
        }
    }

    eprintln!(
        "points={total} formulaA_mismatches={mismatch_a} formulaB_mismatches={mismatch_b}"
    );
}
```

`spikes/win-probes/src/bin/mf_probe.rs`:

```rust
//! Spike only (throwaway): can this process create an H.264 encoder MFT and encode frames?
//!
//! Usage: mf_probe [--software]
//! - default: first hardware encoder MFT (async), D3D11 NV12 texture input via DXGI device manager.
//! - --software: first non-hardware encoder MFT (sync), system-memory NV12 input.
//! Prints the running user and session first so a SYSTEM run (PsExec -s -i) can be told apart.
//! Last line: RESULT mode=.. encoder=".." frames_in=.. frames_out=.. bytes_out=.. first_output_ms=..

use std::mem::ManuallyDrop;
use std::time::{Duration, Instant};

use windows::core::{Interface, Result, GUID, PWSTR};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D10::ID3D10Multithread;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11Texture2D, D3D11_BIND_RENDER_TARGET,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_VIDEO_SUPPORT, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_NV12, DXGI_SAMPLE_DESC};
use windows::Win32::Media::MediaFoundation::{
    IMFActivate, IMFAttributes, IMFDXGIDeviceManager, IMFMediaEventGenerator, IMFSample,
    IMFTransform, MFCreateDXGIDeviceManager, MFCreateDXGISurfaceBuffer, MFCreateMediaType,
    MFCreateMemoryBuffer, MFCreateSample, MFMediaType_Video, MFStartup, MFTEnumEx,
    MFVideoFormat_H264, MFVideoFormat_NV12, MFVideoInterlace_Progressive, METransformHaveOutput,
    METransformNeedInput, MFSTARTUP_FULL, MFT_CATEGORY_VIDEO_ENCODER, MFT_ENUM_FLAG_HARDWARE,
    MFT_ENUM_FLAG_SORTANDFILTER, MFT_ENUM_HARDWARE_URL_Attribute, MFT_FRIENDLY_NAME_Attribute,
    MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, MFT_MESSAGE_NOTIFY_START_OF_STREAM,
    MFT_MESSAGE_SET_D3D_MANAGER, MFT_OUTPUT_DATA_BUFFER, MFT_OUTPUT_STREAM_CAN_PROVIDE_SAMPLES,
    MFT_OUTPUT_STREAM_PROVIDES_SAMPLES, MFT_REGISTER_TYPE_INFO, MF_EVENT_FLAG_NO_WAIT,
    MF_E_TRANSFORM_NEED_MORE_INPUT, MF_MT_AVG_BITRATE, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE,
    MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE, MF_TRANSFORM_ASYNC_UNLOCK, MF_VERSION,
};
use windows::Win32::System::Com::{CoInitializeEx, CoTaskMemFree, COINIT_MULTITHREADED};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::System::WindowsProgramming::GetUserNameW;

const W: u32 = 1920;
const H: u32 = 1080;
const FRAMES: u32 = 30;
const FRAME_100NS: i64 = 10_000_000 / 30;
const TIMEOUT: Duration = Duration::from_secs(5);

fn current_user() -> String {
    let mut buf = [0u16; 256];
    let mut len = buf.len() as u32;
    unsafe {
        match GetUserNameW(Some(PWSTR(buf.as_mut_ptr())), &mut len) {
            Ok(()) => String::from_utf16_lossy(&buf[..(len as usize).saturating_sub(1)]),
            Err(e) => format!("<err {e}>"),
        }
    }
}

fn current_session() -> u32 {
    let mut sid = u32::MAX;
    unsafe {
        let _ = ProcessIdToSessionId(GetCurrentProcessId(), &mut sid);
    }
    sid
}

unsafe fn allocated_string(act: &IMFActivate, key: &GUID) -> Option<String> {
    let mut p = PWSTR::null();
    let mut len = 0u32;
    unsafe {
        act.GetAllocatedString(key, &mut p, &mut len).ok()?;
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(p.0, len as usize));
        CoTaskMemFree(Some(p.0 as *const _));
        Some(s)
    }
}

/// All H.264 encoder activates, hardware ones first when `hardware_only` is false.
unsafe fn enum_h264(hardware_only: bool) -> Result<Vec<IMFActivate>> {
    let out_info = MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Video,
        guidSubtype: MFVideoFormat_H264,
    };
    let mut flags = MFT_ENUM_FLAG_SORTANDFILTER;
    if hardware_only {
        flags |= MFT_ENUM_FLAG_HARDWARE;
    }
    let mut arr: *mut Option<IMFActivate> = std::ptr::null_mut();
    let mut count = 0u32;
    unsafe {
        MFTEnumEx(MFT_CATEGORY_VIDEO_ENCODER, flags, None, Some(&out_info), &mut arr, &mut count)?;
        let list = if arr.is_null() {
            Vec::new()
        } else {
            std::slice::from_raw_parts(arr, count as usize).iter().flatten().cloned().collect()
        };
        if !arr.is_null() {
            CoTaskMemFree(Some(arr as *const _));
        }
        Ok(list)
    }
}

fn pack_u64(hi: u32, lo: u32) -> u64 {
    ((hi as u64) << 32) | lo as u64
}

fn step(label: &str, r: Result<()>) -> Result<()> {
    match &r {
        Ok(()) => println!("  {label}: OK"),
        Err(e) => println!("  {label}: HRESULT {:#010x} ({})", e.code().0, e.message()),
    }
    r
}

unsafe fn set_types(t: &IMFTransform) -> Result<()> {
    unsafe {
        let out_type = MFCreateMediaType()?;
        out_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        out_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
        out_type.SetUINT32(&MF_MT_AVG_BITRATE, 8_000_000)?;
        out_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        out_type.SetUINT64(&MF_MT_FRAME_SIZE, pack_u64(W, H))?;
        out_type.SetUINT64(&MF_MT_FRAME_RATE, pack_u64(30, 1))?;
        // Encoders need the output type before the input type.
        step("SetOutputType(H264 1920x1080 30fps)", t.SetOutputType(0, &out_type, 0))?;

        let in_type = MFCreateMediaType()?;
        in_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        in_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
        in_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        in_type.SetUINT64(&MF_MT_FRAME_SIZE, pack_u64(W, H))?;
        in_type.SetUINT64(&MF_MT_FRAME_RATE, pack_u64(30, 1))?;
        step("SetInputType(NV12)", t.SetInputType(0, &in_type, 0))
    }
}

struct Counters {
    frames_in: u32,
    frames_out: u32,
    bytes_out: u64,
    first_output: Option<Duration>,
    started: Instant,
}

/// Pull one output. Ok(false) means the MFT needs more input.
unsafe fn pull_output(t: &IMFTransform, provides_samples: bool, out_size: u32, c: &mut Counters) -> Result<bool> {
    unsafe {
        let own_sample: Option<IMFSample> = if provides_samples {
            None
        } else {
            let s = MFCreateSample()?;
            s.AddBuffer(&MFCreateMemoryBuffer(out_size.max(W * H))?)?;
            Some(s)
        };
        let mut bufs = [MFT_OUTPUT_DATA_BUFFER {
            dwStreamID: 0,
            pSample: ManuallyDrop::new(own_sample),
            dwStatus: 0,
            pEvents: ManuallyDrop::new(None),
        }];
        let mut status = 0u32;
        let r = t.ProcessOutput(0, &mut bufs, &mut status);
        let sample = ManuallyDrop::take(&mut bufs[0].pSample);
        drop(ManuallyDrop::take(&mut bufs[0].pEvents));
        match r {
            Ok(()) => {
                if let Some(s) = sample {
                    c.frames_out += 1;
                    c.bytes_out += s.GetTotalLength()? as u64;
                    c.first_output.get_or_insert(c.started.elapsed());
                }
                Ok(true)
            }
            Err(e) if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => Ok(false),
            Err(e) => Err(e),
        }
    }
}

unsafe fn output_info(t: &IMFTransform) -> Result<(bool, u32)> {
    unsafe {
        let info = t.GetOutputStreamInfo(0)?;
        let provides = info.dwFlags
            & (MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 as u32 | MFT_OUTPUT_STREAM_CAN_PROVIDE_SAMPLES.0 as u32)
            != 0;
        println!("  output stream: provides_samples={provides} cbSize={}", info.cbSize);
        Ok((provides, info.cbSize))
    }
}

unsafe fn begin_streaming(t: &IMFTransform) -> Result<()> {
    unsafe {
        step("NOTIFY_BEGIN_STREAMING", t.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0))?;
        step("NOTIFY_START_OF_STREAM", t.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0))
    }
}

/// Hardware path: async MFT, D3D11 NV12 texture input.
unsafe fn run_hardware(act: &IMFActivate, c: &mut Counters) -> Result<()> {
    unsafe {
        let attrs = act.cast::<IMFAttributes>()?;
        let _ = step("MF_TRANSFORM_ASYNC_UNLOCK", attrs.SetUINT32(&MF_TRANSFORM_ASYNC_UNLOCK, 1));
        let t: IMFTransform = act.ActivateObject()?;
        // Unlock again on the transform's own attribute store (async MFTs check it there).
        if let Ok(ta) = t.GetAttributes() {
            let _ = step("transform MF_TRANSFORM_ASYNC_UNLOCK", ta.SetUINT32(&MF_TRANSFORM_ASYNC_UNLOCK, 1));
        }

        let mut device: Option<ID3D11Device> = None;
        step(
            "D3D11CreateDevice(VIDEO|BGRA)",
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_VIDEO_SUPPORT | D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                None,
            ),
        )?;
        let device = device.expect("device");
        let _ = device.cast::<ID3D10Multithread>()?.SetMultithreadProtected(true);

        let mut token = 0u32;
        let mut manager: Option<IMFDXGIDeviceManager> = None;
        step("MFCreateDXGIDeviceManager", MFCreateDXGIDeviceManager(&mut token, &mut manager))?;
        let manager = manager.expect("manager");
        step("ResetDevice", manager.ResetDevice(&device, token))?;
        step("SET_D3D_MANAGER", t.ProcessMessage(MFT_MESSAGE_SET_D3D_MANAGER, manager.as_raw() as usize))?;

        set_types(&t)?;
        let (provides, out_size) = output_info(&t)?;

        let mut desc = D3D11_TEXTURE2D_DESC {
            Width: W,
            Height: H,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_NV12,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let mut tex: Option<ID3D11Texture2D> = None;
        if step("CreateTexture2D(NV12, RENDER_TARGET)", device.CreateTexture2D(&desc, None, Some(&mut tex))).is_err() {
            desc.BindFlags = 0;
            step("CreateTexture2D(NV12, no bind flags)", device.CreateTexture2D(&desc, None, Some(&mut tex)))?;
        }
        let tex = tex.expect("texture");

        let events = t.cast::<IMFMediaEventGenerator>()?;
        begin_streaming(&t)?;
        c.started = Instant::now();
        while c.frames_out < FRAMES && c.started.elapsed() < TIMEOUT {
            let ev = match events.GetEvent(MF_EVENT_FLAG_NO_WAIT) {
                Ok(ev) => ev,
                Err(_) => {
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
            };
            let kind = ev.GetType()?;
            if kind == METransformNeedInput.0 as u32 && c.frames_in < FRAMES {
                let buffer = MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, &tex, 0, false)?;
                let sample = MFCreateSample()?;
                sample.AddBuffer(&buffer)?;
                sample.SetSampleTime(c.frames_in as i64 * FRAME_100NS)?;
                sample.SetSampleDuration(FRAME_100NS)?;
                t.ProcessInput(0, &sample, 0)?;
                c.frames_in += 1;
            } else if kind == METransformHaveOutput.0 as u32 {
                pull_output(&t, provides, out_size, c)?;
            }
        }
        Ok(())
    }
}

/// Software path: sync MFT, system-memory NV12 input.
unsafe fn run_software(act: &IMFActivate, c: &mut Counters) -> Result<()> {
    unsafe {
        let t: IMFTransform = act.ActivateObject()?;
        set_types(&t)?;
        let (provides, out_size) = output_info(&t)?;
        begin_streaming(&t)?;
        let frame_bytes = W * H * 3 / 2;
        c.started = Instant::now();
        while c.frames_in < FRAMES && c.started.elapsed() < TIMEOUT {
            let buffer = MFCreateMemoryBuffer(frame_bytes)?;
            let mut ptr: *mut u8 = std::ptr::null_mut();
            buffer.Lock(&mut ptr, None, None)?;
            // Mid-grey frame with a moving bright band so frames differ.
            let data = std::slice::from_raw_parts_mut(ptr, frame_bytes as usize);
            data.fill(128);
            let band = (c.frames_in * 32 % H) as usize;
            data[band * W as usize..(band + 16) * W as usize].fill(235);
            buffer.Unlock()?;
            buffer.SetCurrentLength(frame_bytes)?;
            let sample = MFCreateSample()?;
            sample.AddBuffer(&buffer)?;
            sample.SetSampleTime(c.frames_in as i64 * FRAME_100NS)?;
            sample.SetSampleDuration(FRAME_100NS)?;
            t.ProcessInput(0, &sample, 0)?;
            c.frames_in += 1;
            while pull_output(&t, provides, out_size, c)? {}
        }
        Ok(())
    }
}

fn main() -> Result<()> {
    let software = std::env::args().any(|a| a == "--software");
    println!("user={} session={}", current_user(), current_session());
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        MFStartup(MF_VERSION, MFSTARTUP_FULL)?;

        let all = enum_h264(false)?;
        println!("H.264 encoders: {}", all.len());
        for a in &all {
            let name = allocated_string(a, &MFT_FRIENDLY_NAME_Attribute).unwrap_or_default();
            let hw = allocated_string(a, &MFT_ENUM_HARDWARE_URL_Attribute).is_some();
            println!("  - {name} (hardware={hw})");
        }

        let pick = if software {
            all.iter().find(|a| allocated_string(a, &MFT_ENUM_HARDWARE_URL_Attribute).is_none()).cloned()
        } else {
            enum_h264(true)?.into_iter().next()
        };
        let mode = if software { "software" } else { "hardware" };
        let Some(act) = pick else {
            println!("RESULT mode={mode} encoder=\"<none>\" frames_in=0 frames_out=0 bytes_out=0 first_output_ms=-");
            return Ok(());
        };
        let name = allocated_string(&act, &MFT_FRIENDLY_NAME_Attribute).unwrap_or_default();
        println!("using: {name}");

        let mut c = Counters { frames_in: 0, frames_out: 0, bytes_out: 0, first_output: None, started: Instant::now() };
        let r = if software { run_software(&act, &mut c) } else { run_hardware(&act, &mut c) };
        if let Err(e) = &r {
            println!("  encode loop error: HRESULT {:#010x} ({})", e.code().0, e.message());
        }
        let first = c.first_output.map(|d| format!("{}", d.as_millis())).unwrap_or_else(|| "-".into());
        println!(
            "RESULT mode={mode} encoder=\"{name}\" frames_in={} frames_out={} bytes_out={} first_output_ms={first}",
            c.frames_in, c.frames_out, c.bytes_out
        );
    }
    Ok(())
}
```

---

## 부록 C. `spikes/auth`

`spikes/auth/Cargo.toml`:

```toml
[package]
name = "spike-auth"
version = "0.0.0"
edition = "2024"
publish = false

[dev-dependencies]
spake2 = "=0.4.0"
rand_core_06 = { package = "rand_core", version = "0.6", features = ["getrandom"] }
pakery-spake2 = { version = "=0.6.0", features = ["test-utils"] }
pakery-core = "=0.6.0"
pakery-crypto = { version = "=0.6.0", default-features = false, features = ["std", "p256", "spake2"] }
rand_core = "0.10"
getrandom = { version = "0.4", features = ["sys_rng"] }
snow = "=0.10.0"
hex = "0.4"
num-bigint = "0.4"
```

`spikes/auth/src/lib.rs`:

```rust
//! Spike only (throwaway): tests live in tests/.
```

`spikes/auth/tests/pakery_rfc9382.rs`:

```rust
// Spike only (throwaway).
use pakery_core::crypto::{CpaceGroup, Hash, Kdf, Mac};
use pakery_crypto::{HkdfSha256, HmacSha256, P256Group, Sha256Hash, Sha512Hash, Spake2P256};
use pakery_spake2::encoding::build_transcript;
use pakery_spake2::{PartyA, PartyB, Spake2Error};

type A = PartyA<Spake2P256>;
type B = PartyB<Spake2P256>;
type Scalar = <P256Group as CpaceGroup>::Scalar;

fn h(s: &str) -> Vec<u8> {
    hex::decode(s).unwrap()
}

fn pw(p: &[u8]) -> Scalar {
    P256Group::scalar_from_wide_bytes(&Sha512Hash::digest(p)).unwrap()
}

/// 32-byte big-endian RFC scalar -> P-256 scalar through the public API only:
/// scalar_from_wide_bytes reads 64 bytes big-endian and reduces mod n, so
/// 32 zero bytes || s gives s itself (all vector scalars are < n).
fn scalar(s: &str) -> Scalar {
    let mut wide = vec![0u8; 32];
    wide.extend_from_slice(&h(s));
    let sc = P256Group::scalar_from_wide_bytes(&wide).unwrap();
    assert_eq!(hex::encode(P256Group::scalar_to_bytes(&sc)), s);
    sc
}

#[test]
fn a_same_password_mutual_confirmation() {
    let mut rng = rand_core::UnwrapErr(getrandom::SysRng);
    let w = pw(b"123456");
    let (pa, sa) = A::start(&w, b"host", b"viewer", b"aad", &mut rng).unwrap();
    let (pb, sb) = B::start(&w, b"host", b"viewer", b"aad", &mut rng).unwrap();
    assert_eq!(pa.len(), 65);
    let oa = sa.finish(&pb).unwrap();
    let ob = sb.finish(&pa).unwrap();
    assert_eq!(oa.session_key.as_bytes(), ob.session_key.as_bytes());
    assert_eq!(oa.session_key.as_bytes().len(), 16); // Ke = NH/2 for SHA-256
    oa.verify_peer_confirmation(&ob.confirmation_mac).unwrap();
    ob.verify_peer_confirmation(&oa.confirmation_mac).unwrap();
}

#[test]
fn b_wrong_password_confirmation_fails() {
    let mut rng = rand_core::UnwrapErr(getrandom::SysRng);
    let (pa, sa) = A::start(&pw(b"123456"), b"host", b"viewer", b"", &mut rng).unwrap();
    let (pb, sb) = B::start(&pw(b"654321"), b"host", b"viewer", b"", &mut rng).unwrap();
    let oa = sa.finish(&pb).unwrap(); // finish itself succeeds
    let ob = sb.finish(&pa).unwrap();
    assert_ne!(oa.session_key.as_bytes(), ob.session_key.as_bytes());
    assert!(matches!(
        oa.verify_peer_confirmation(&ob.confirmation_mac),
        Err(Spake2Error::ConfirmationFailed)
    ));
    assert!(matches!(
        ob.verify_peer_confirmation(&oa.confirmation_mac),
        Err(Spake2Error::ConfirmationFailed)
    ));
}

// RFC 9382 Appendix B, first vector (A="server", B="client"),
// values copied from upstream pakery-tests/tests/spake2_p256_vectors.rs:134-145.
const W: &str = "2ee57912099d31560b3a44b1184b9b4866e904c49d12ac5042c97dca461b1a5f";
const X: &str = "43dd0fd7215bdcb482879fca3220c6a968e66d70b1356cac18bb26c84a78d729";
const Y: &str = "dcb60106f276b02606d8ef0a328c02e4b629f84f89786af5befb0bc75b6e66be";
const PA: &str = "04a56fa807caaa53a4d28dbb9853b9815c61a411118a6fe516a8798434751470f9010153ac33d0d5f2047ffdb1a3e42c9b4e6be662766e1eeb4116988ede5f912c";
const PB: &str = "0406557e482bd03097ad0cbaa5df82115460d951e3451962f1eaf4367a420676d09857ccbc522686c83d1852abfa8ed6e4a1155cf8f1543ceca528afb591a1e0b7";
const K: &str = "0412af7e89717850671913e6b469ace67bd90a4df8ce45c2af19010175e37eed69f75897996d539356e2fa6a406d528501f907e04d97515fbe83db277b715d3325";
const HASH_TT: &str = "0e0672dc86f8e45565d338b0540abe6915bdf72e2b35b5c9e5663168e960a91b";
const KE: &str = "0e0672dc86f8e45565d338b0540abe69";
const KCA: &str = "00c12546835755c86d8c0db7851ae86f";
const KCB: &str = "a9fa3406c3b781b93d804485430ca27a";
const MAC_A: &str = "58ad4aa88e0b60d5061eb6b5dd93e80d9c4f00d127c65b3b35b1b5281fee38f0";
const MAC_B: &str = "d3e2e547f1ae04f2dbdbf0fc4b79f8ecff2dff314b5d32fe9fcef2fb26dc459b";

#[test]
fn c_rfc9382_vector1_full() {
    let (w, x, y) = (scalar(W), scalar(X), scalar(Y));
    let (pa, sa) = A::start_with_scalar(&w, &x, b"server", b"client", b"").unwrap();
    let (pb, sb) = B::start_with_scalar(&w, &y, b"server", b"client", b"").unwrap();
    assert_eq!(hex::encode(&pa), PA);
    assert_eq!(hex::encode(&pb), PB);

    // K is not exposed by PartyXState::finish; recompute K = x*(pB - w*N)
    // with the public group API and check it, then rebuild TT with the
    // crate's own public build_transcript.
    let n = P256Group::from_bytes(<Spake2P256 as pakery_spake2::Spake2Ciphersuite>::N_BYTES).unwrap();
    let k = P256Group::from_bytes(&pb).unwrap().add(&n.scalar_mul(&w).negate()).scalar_mul(&x);
    assert_eq!(hex::encode(k.to_bytes()), K);
    let tt = build_transcript(b"server", b"client", &pa, &pb, &k.to_bytes(), &P256Group::scalar_to_bytes(&w));
    let hash_tt = Sha256Hash::digest(&tt);
    assert_eq!(hex::encode(&hash_tt), HASH_TT);
    let prk = HkdfSha256::extract(&[], &hash_tt[16..]);
    let kc = HkdfSha256::expand(&prk, b"ConfirmationKeys", 32).unwrap();
    assert_eq!(hex::encode(&kc[..16]), KCA);
    assert_eq!(hex::encode(&kc[16..]), KCB);
    assert_eq!(hex::encode(HmacSha256::mac(&kc[..16], &tt).unwrap()), MAC_A);
    assert_eq!(hex::encode(HmacSha256::mac(&kc[16..], &tt).unwrap()), MAC_B);

    // The protocol path itself.
    let oa = sa.finish(&pb).unwrap();
    let ob = sb.finish(&pa).unwrap();
    assert_eq!(hex::encode(oa.session_key.as_bytes()), KE);
    assert_eq!(hex::encode(ob.session_key.as_bytes()), KE);
    assert_eq!(hex::encode(&oa.confirmation_mac), MAC_A);
    assert_eq!(hex::encode(&ob.confirmation_mac), MAC_B);
    oa.verify_peer_confirmation(&ob.confirmation_mac).unwrap();
    ob.verify_peer_confirmation(&oa.confirmation_mac).unwrap();
}
```

`spikes/auth/tests/spake2_crate.rs`:

```rust
// Spike only (throwaway).
use num_bigint::BigUint;
use rand_core_06::{CryptoRng, RngCore};
use spake2::{Ed25519Group, Error, Identity, Password, Spake2};

fn start_pair(pw_a: &[u8], pw_b: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let id_a = Identity::new(b"viewer");
    let id_b = Identity::new(b"host");
    let (sa, msg_a) = Spake2::<Ed25519Group>::start_a(&Password::new(pw_a), &id_a, &id_b);
    let (sb, msg_b) = Spake2::<Ed25519Group>::start_b(&Password::new(pw_b), &id_a, &id_b);
    let ka = sa.finish(&msg_b).unwrap();
    let kb = sb.finish(&msg_a).unwrap();
    (ka, kb)
}

#[test]
fn same_code_gives_equal_32_byte_keys() {
    let (ka, kb) = start_pair(b"123456", b"123456");
    assert_eq!(ka.len(), 32);
    assert_eq!(ka, kb);
}

#[test]
fn different_code_gives_different_keys() {
    let (ka, kb) = start_pair(b"123456", b"123457");
    assert_ne!(ka, kb);
}

#[test]
fn symmetric_mode_roundtrip() {
    let id = Identity::new(b"simple-remote pairing");
    let (s1, m1) = Spake2::<Ed25519Group>::start_symmetric(&Password::new(b"123456"), &id);
    let (s2, m2) = Spake2::<Ed25519Group>::start_symmetric(&Password::new(b"123456"), &id);
    assert_eq!(s1.finish(&m2).unwrap(), s2.finish(&m1).unwrap());
}

#[test]
fn reflected_message_is_rejected() {
    let (s1, m1) = Spake2::<Ed25519Group>::start_a(
        &Password::new(b"123456"),
        &Identity::new(b"viewer"),
        &Identity::new(b"host"),
    );
    assert_eq!(s1.finish(&m1).unwrap_err(), Error::BadSide);
}

/// RNG that yields `scalar_le || 0^32`. Curve25519 `Scalar::random` reads 64 bytes and
/// reduces mod l (from_bytes_mod_order_wide), so this makes the ephemeral scalar exactly
/// `scalar_le` when it is already < l.
struct FixedScalarRng {
    buf: [u8; 64],
}
impl FixedScalarRng {
    fn from_decimal(d: &[u8]) -> Self {
        let le = BigUint::parse_bytes(d, 10).unwrap().to_bytes_le();
        let mut buf = [0u8; 64];
        buf[..le.len()].copy_from_slice(&le);
        Self { buf }
    }
}
impl RngCore for FixedScalarRng {
    fn next_u32(&mut self) -> u32 {
        unimplemented!()
    }
    fn next_u64(&mut self) -> u64 {
        unimplemented!()
    }
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        assert_eq!(dest.len(), 64);
        dest.copy_from_slice(&self.buf);
    }
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core_06::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}
impl CryptoRng for FixedScalarRng {}

/// Same vector as the crate's private `tests::test_asymmetric` (src/lib.rs:700-757), which
/// comes from python-spake2 `test_compat` (NOT RFC 9382). Reproduced here via the public
/// `start_*_with_rng` API.
#[test]
fn python_spake2_compat_vector_via_public_api() {
    let rng_a = FixedScalarRng::from_decimal(
        b"2611694063369306139794446498317402240796898290761098242657700742213257926693",
    );
    let rng_b = FixedScalarRng::from_decimal(
        b"7002393159576182977806091886122272758628412261510164356026361256515836884383",
    );
    let pw = Password::new(b"password");
    let (id_a, id_b) = (Identity::new(b"idA"), Identity::new(b"idB"));

    let (s1, msg1) = Spake2::<Ed25519Group>::start_a_with_rng(&pw, &id_a, &id_b, rng_a);
    assert_eq!(
        hex::encode(&msg1),
        "416fc960df73c9cf8ed7198b0c9534e2e96a5984bfc5edc023fd24dacf371f2af9"
    );
    let (s2, msg2) = Spake2::<Ed25519Group>::start_b_with_rng(&pw, &id_a, &id_b, rng_b);
    assert_eq!(
        hex::encode(&msg2),
        "42354e97b88406922b1df4bea1d7870f17aed3dba7c720b313edae315b00959309"
    );
    let k1 = s1.finish(&msg2).unwrap();
    let k2 = s2.finish(&msg1).unwrap();
    assert_eq!(k1, k2);
    assert_eq!(
        hex::encode(k1),
        "712295de7219c675ddd31942184aa26e0a957cf216bc230d165b215047b520c1"
    );
}
```

`spikes/auth/tests/noise_kk.rs`:

```rust
// Spike only (throwaway).
use snow::{Builder, HandshakeState, Keypair};

const PARAMS: &str = "Noise_KK_25519_ChaChaPoly_BLAKE2s";

fn keypair() -> Keypair {
    Builder::new(PARAMS.parse().unwrap()).generate_keypair().unwrap()
}

fn build(local: &Keypair, remote_pub: &[u8], initiator: bool) -> HandshakeState {
    let b = Builder::new(PARAMS.parse().unwrap())
        .prologue(b"simple-remote v1")
        .unwrap()
        .local_private_key(&local.private)
        .unwrap()
        .remote_public_key(remote_pub)
        .unwrap();
    if initiator {
        b.build_initiator().unwrap()
    } else {
        b.build_responder().unwrap()
    }
}

#[test]
fn kk_handshake_then_transport_both_ways() {
    let (ik, rk) = (keypair(), keypair());
    let mut ini = build(&ik, &rk.public, true);
    let mut res = build(&rk, &ik.public, false);
    let (mut msg, mut out) = (vec![0u8; 65535], vec![0u8; 65535]);

    // -> e, es, ss
    let n = ini.write_message(b"hello", &mut msg).unwrap();
    let p = res.read_message(&msg[..n], &mut out).unwrap();
    assert_eq!(&out[..p], b"hello");
    // <- e, ee, se
    let n = res.write_message(b"", &mut msg).unwrap();
    let p = ini.read_message(&msg[..n], &mut out).unwrap();
    assert_eq!(p, 0);

    assert!(ini.is_handshake_finished() && res.is_handshake_finished());
    assert_eq!(ini.get_handshake_hash(), res.get_handshake_hash());
    assert_eq!(ini.get_handshake_hash().len(), 32); // BLAKE2s HASHLEN
    assert_eq!(ini.get_remote_static().unwrap(), &rk.public[..]);

    let mut ti = ini.into_transport_mode().unwrap();
    let mut tr = res.into_transport_mode().unwrap();

    let n = ti.write_message(b"ping", &mut msg).unwrap();
    assert_eq!(n, 4 + 16); // payload + AEAD tag
    let p = tr.read_message(&msg[..n], &mut out).unwrap();
    assert_eq!(&out[..p], b"ping");

    let n = tr.write_message(b"pong", &mut msg).unwrap();
    let p = ti.read_message(&msg[..n], &mut out).unwrap();
    assert_eq!(&out[..p], b"pong");
}

#[test]
fn wrong_remote_static_fails_handshake() {
    let (ik, rk, other) = (keypair(), keypair(), keypair());
    // Initiator believes the responder's static key is `other`, not `rk`.
    let mut ini = build(&ik, &other.public, true);
    let mut res = build(&rk, &ik.public, false);
    let (mut msg, mut out) = (vec![0u8; 65535], vec![0u8; 65535]);

    let n = ini.write_message(b"hello", &mut msg).unwrap();
    // es/ss mismatch -> payload AEAD fails on the responder
    assert!(matches!(
        res.read_message(&msg[..n], &mut out),
        Err(snow::Error::Decrypt)
    ));
}

#[test]
fn responder_expecting_wrong_initiator_static_fails() {
    let (ik, rk, other) = (keypair(), keypair(), keypair());
    let mut ini = build(&ik, &rk.public, true);
    let mut res = build(&rk, &other.public, false);
    let (mut msg, mut out) = (vec![0u8; 65535], vec![0u8; 65535]);

    let n = ini.write_message(b"", &mut msg).unwrap();
    // Even with an empty payload, the first message carries a 16-byte tag after es+ss.
    assert_eq!(n, 32 + 16);
    assert!(matches!(
        res.read_message(&msg[..n], &mut out),
        Err(snow::Error::Decrypt)
    ));
}
```

---

## 부록 D. `spikes/webrtc-str0m`

`spikes/webrtc-str0m/Cargo.toml`:

```toml
[package]
name = "spike-webrtc-str0m"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
str0m = { version = "=0.23.1", default-features = false, features = ["aws-lc-rs"] }
```

`spikes/webrtc-str0m/src/lib.rs`:

```rust
// Spike only (throwaway).
//! Spike package: the work lives in `tests/`. Run with
//! `cargo test -- --nocapture --test-threads=1`.
```

`spikes/webrtc-str0m/tests/common/mod.rs`:

```rust
// Spike only (throwaway).
//! In-memory two-peer harness for str0m spikes.
//!
//! - No sockets. Time is simulated: `Sim::now` only moves when the test calls
//!   `advance_to` / `run_for` / `run_until`.
//! - Two one-way links (L->R, R->L). Each link has fixed latency, an optional
//!   bottleneck (serialization at `rate_bps` with a tail-drop byte queue), and an
//!   optional deterministic "drop every n-th packet" rule.
//! - A NAT table (internal <-> external) rewrites addresses on every packet, and a
//!   `blocked` list drops packets to or from addresses that no longer exist.
#![allow(dead_code)]

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use str0m::change::{SdpAnswer, SdpApi, SdpOffer};
use str0m::net::{Protocol, Receive};
use str0m::{Event, Input, Output, Rtc, RtcError};

#[derive(Debug, Clone)]
pub struct Packet {
    pub source: SocketAddr,
    pub destination: SocketAddr,
    pub contents: Vec<u8>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct LinkStats {
    pub sent: u64,
    pub delivered: u64,
    pub dropped_loss: u64,
    pub dropped_queue: u64,
    pub dropped_blocked: u64,
}

/// One direction of the simulated network path.
#[derive(Default)]
pub struct Link {
    pub latency: Duration,
    /// Bottleneck capacity. `None` = infinite.
    pub rate_bps: Option<u64>,
    /// Max bytes waiting in the bottleneck queue (tail drop). Only used with `rate_bps`.
    pub queue_limit_bytes: usize,
    /// Drop every n-th packet that enters this link (deterministic loss).
    pub drop_every_nth: Option<u64>,
    loss_counter: u64,
    busy_until: Option<Instant>,
    in_flight: VecDeque<(Instant, Packet)>,
    pub stats: LinkStats,
}

/// IP + UDP header overhead used when charging the bottleneck.
const IP_UDP_OVERHEAD: usize = 28;

impl Link {
    fn push(&mut self, now: Instant, p: Packet) {
        self.stats.sent += 1;

        if let Some(n) = self.drop_every_nth {
            self.loss_counter += 1;
            if self.loss_counter % n == 0 {
                self.stats.dropped_loss += 1;
                return;
            }
        }

        let depart = match self.rate_bps {
            Some(rate) => {
                let start = self.busy_until.map_or(now, |b| b.max(now));
                let backlog_bytes = (start - now).as_secs_f64() * rate as f64 / 8.0;
                let size = p.contents.len() + IP_UDP_OVERHEAD;
                if backlog_bytes + size as f64 > self.queue_limit_bytes as f64 {
                    self.stats.dropped_queue += 1;
                    return;
                }
                let tx = Duration::from_secs_f64(size as f64 * 8.0 / rate as f64);
                self.busy_until = Some(start + tx);
                start + tx
            }
            None => now,
        };

        // Keep FIFO order even if latency was lowered mid-run.
        let mut due = depart + self.latency;
        if let Some((last_due, _)) = self.in_flight.back() {
            due = due.max(*last_due);
        }
        self.in_flight.push_back((due, p));
    }

    fn next_due(&self) -> Option<Instant> {
        self.in_flight.front().map(|(t, _)| *t)
    }

    fn pop_due(&mut self, now: Instant) -> Option<Packet> {
        if self.next_due()? <= now {
            self.stats.delivered += 1;
            self.in_flight.pop_front().map(|(_, p)| p)
        } else {
            None
        }
    }

    /// Current queueing delay at the bottleneck (0 when no bottleneck).
    pub fn queue_delay(&self, now: Instant) -> Duration {
        self.busy_until
            .map_or(Duration::ZERO, |b| b.saturating_duration_since(now))
    }
}

pub struct Peer {
    pub name: &'static str,
    pub rtc: Rtc,
    pub next_timeout: Instant,
    /// Events with the simulated elapsed time at which they were polled.
    pub events: Vec<(Duration, Event)>,
    /// (source, destination) of the most recent datagram this peer transmitted.
    pub last_tx: Option<(SocketAddr, SocketAddr)>,
    /// Print ICE / connection / channel events as they happen.
    pub verbose: bool,
}

impl Peer {
    pub fn new(name: &'static str, rtc: Rtc, now: Instant) -> Self {
        Peer {
            name,
            rtc,
            next_timeout: now,
            events: vec![],
            last_tx: None,
            verbose: true,
        }
    }

    pub fn has_event(&self, f: impl Fn(&Event) -> bool) -> bool {
        self.events.iter().any(|(_, e)| f(e))
    }

    pub fn first_event_time(&self, f: impl Fn(&Event) -> bool) -> Option<Duration> {
        self.events.iter().find(|(_, e)| f(e)).map(|(t, _)| *t)
    }
}

pub struct Sim {
    pub start: Instant,
    pub now: Instant,
    pub l: Peer,
    pub r: Peer,
    pub l_to_r: Link,
    pub r_to_l: Link,
    /// (internal, external). Source `internal` is rewritten to `external`;
    /// destination `external` is rewritten to `internal`.
    pub nat: Vec<(SocketAddr, SocketAddr)>,
    /// Packets whose source or destination is in this list are dropped.
    pub blocked: Vec<SocketAddr>,
    /// Forced time advance when an Rtc asks for a timeout at or before `now`.
    pub min_step: Duration,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    L,
    R,
}

impl Sim {
    pub fn new(now: Instant, l: Rtc, r: Rtc) -> Self {
        Sim {
            start: now,
            now,
            l: Peer::new("L", l, now),
            r: Peer::new("R", r, now),
            l_to_r: Link::default(),
            r_to_l: Link::default(),
            nat: vec![],
            blocked: vec![],
            min_step: Duration::from_millis(1),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.now - self.start
    }

    /// Process every timeout and packet delivery up to and including `target`.
    pub fn advance_to(&mut self, target: Instant) -> Result<(), RtcError> {
        // Anything the test did through the API since the last call (write, sdp change,
        // candidate add) only turns into output after poll_output, so poll both now.
        self.l.next_timeout = self.l.next_timeout.min(self.now);
        self.r.next_timeout = self.r.next_timeout.min(self.now);

        loop {
            let mut next = self.l.next_timeout.min(self.r.next_timeout);
            if let Some(t) = self.l_to_r.next_due() {
                next = next.min(t);
            }
            if let Some(t) = self.r_to_l.next_due() {
                next = next.min(t);
            }
            if next > target {
                break;
            }
            let t = next.max(self.now);
            self.now = t;

            while let Some(p) = self.l_to_r.pop_due(t) {
                receive(&mut self.r, t, &p)?;
                self.drain(Side::R, t)?;
            }
            while let Some(p) = self.r_to_l.pop_due(t) {
                receive(&mut self.l, t, &p)?;
                self.drain(Side::L, t)?;
            }
            if self.l.next_timeout <= t {
                self.l.rtc.handle_input(Input::Timeout(t))?;
                self.drain(Side::L, t)?;
            }
            if self.r.next_timeout <= t {
                self.r.rtc.handle_input(Input::Timeout(t))?;
                self.drain(Side::R, t)?;
            }
        }

        if target > self.now {
            self.now = target;
        }
        Ok(())
    }

    pub fn run_for(&mut self, d: Duration) -> Result<(), RtcError> {
        let target = self.now + d;
        self.advance_to(target)
    }

    /// Advance in `step` increments until `cond` is true or `limit` of simulated time
    /// has passed. Returns the elapsed simulated time (since this call) when `cond` held.
    pub fn run_until(
        &mut self,
        limit: Duration,
        step: Duration,
        mut cond: impl FnMut(&mut Sim) -> bool,
    ) -> Result<Option<Duration>, RtcError> {
        let begin = self.now;
        loop {
            if cond(self) {
                return Ok(Some(self.now - begin));
            }
            if self.now - begin >= limit {
                return Ok(None);
            }
            self.run_for(step)?;
        }
    }

    fn drain(&mut self, side: Side, now: Instant) -> Result<(), RtcError> {
        let start = self.start;
        let min_step = self.min_step;
        let (peer, link) = match side {
            Side::L => (&mut self.l, &mut self.l_to_r),
            Side::R => (&mut self.r, &mut self.r_to_l),
        };
        loop {
            match peer.rtc.poll_output()? {
                Output::Timeout(v) => {
                    peer.next_timeout = if v <= now { now + min_step } else { v };
                    return Ok(());
                }
                Output::Transmit(t) => {
                    peer.last_tx = Some((t.source, t.destination));
                    let mut p = Packet {
                        source: t.source,
                        destination: t.destination,
                        contents: t.contents.to_vec(),
                    };
                    if self.blocked.contains(&p.source) || self.blocked.contains(&p.destination)
                    {
                        link.stats.sent += 1;
                        link.stats.dropped_blocked += 1;
                        continue;
                    }
                    for (internal, external) in &self.nat {
                        if p.source == *internal {
                            p.source = *external;
                        }
                        if p.destination == *external {
                            p.destination = *internal;
                        }
                    }
                    link.push(now, p);
                }
                Output::Event(e) => {
                    if peer.verbose {
                        log_event(peer.name, now - start, &e);
                    }
                    peer.events.push((now - start, e));
                }
            }
        }
    }
}

fn receive(peer: &mut Peer, now: Instant, p: &Packet) -> Result<(), RtcError> {
    let input = Input::Receive(
        now,
        Receive {
            proto: Protocol::Udp,
            source: p.source,
            destination: p.destination,
            contents: p.contents.as_slice().try_into()?,
        },
    );
    peer.rtc.handle_input(input)
}

fn log_event(name: &str, t: Duration, e: &Event) {
    match e {
        Event::IceConnectionStateChange(s) => println!("[{t:>10.3?}] {name} ICE {s:?}"),
        Event::Connected => println!("[{t:>10.3?}] {name} Connected (ICE+DTLS)"),
        Event::ChannelOpen(id, label) => {
            println!("[{t:>10.3?}] {name} ChannelOpen {id:?} {label:?}")
        }
        Event::ChannelClose(id) => println!("[{t:>10.3?}] {name} ChannelClose {id:?}"),
        Event::MediaAdded(m) => println!("[{t:>10.3?}] {name} MediaAdded {m:?}"),
        Event::KeyframeRequest(k) => println!("[{t:>10.3?}] {name} KeyframeRequest {k:?}"),
        _ => {}
    }
}

/// SDP offer/answer through strings, as a signaling server would carry them.
/// Returns (closure result, offer sdp, answer sdp).
pub fn negotiate<T>(
    offerer: &mut Rtc,
    answerer: &mut Rtc,
    change: impl FnOnce(&mut SdpApi) -> T,
) -> Result<(T, String, String), RtcError> {
    let mut api = offerer.sdp_api();
    let out = change(&mut api);
    let (offer, pending) = api.apply().expect("changes that need negotiation");
    let offer_sdp = offer.to_sdp_string();

    let offer = SdpOffer::from_sdp_string(&offer_sdp)?;
    let answer = answerer.sdp_api().accept_offer(offer)?;
    let answer_sdp = answer.to_sdp_string();

    let answer = SdpAnswer::from_sdp_string(&answer_sdp)?;
    offerer.sdp_api().accept_answer(pending, answer)?;
    Ok((out, offer_sdp, answer_sdp))
}

pub fn addr(s: &str) -> SocketAddr {
    s.parse().unwrap()
}

pub fn both_connected(sim: &Sim) -> bool {
    sim.l.rtc.is_connected() && sim.r.rtc.is_connected()
}
```

`spikes/webrtc-str0m/tests/external_candidate.rs`:

```rust
// Spike only (throwaway).
//! Q1. Can the host side (R) advertise only a server-reflexive candidate that it
//! learned out of band (for example a UPnP-mapped public address) and still connect?
//!
//! Network: R's socket is bound to BASE (127.0.0.1:10002). A NAT maps it to
//! MAPPED (127.0.0.2:40000). Packets to MAPPED reach R with destination=BASE;
//! packets from BASE appear to L as coming from MAPPED.
//!
//! Source facts this test relies on (is-0.11.0):
//! - agent.rs `stun_server_handle_request`: an incoming STUN request is only accepted
//!   when its destination equals a local candidate of kind Host or Relayed. A srflx
//!   candidate alone therefore cannot receive L's checks: R must ALSO hold a local host
//!   candidate for BASE. We add that host candidate after the SDP answer was created,
//!   so it is never signalled (L only ever learns MAPPED).
//! - agent.rs `form_pairs`: pairs whose local candidates share the same base and whose
//!   remote is identical are redundant; the higher priority (host) one is kept.
//!   Sending always uses `local.base()` (agent.rs NominatedSend), so R sends from BASE.
//! - Selected pair observability: there is no dedicated event. `Event::PeerStats`
//!   (enabled by `RtcConfig::set_stats_interval`) carries `selected_candidate_pair`
//!   built from the nominated send address (lib.rs `do_handle_timeout`): local.addr is
//!   the send *base*, remote.addr the destination. The last `Output::Transmit`
//!   (source, destination) is a second, direct observation.
mod common;

use std::time::Duration;

use common::{Sim, addr, both_connected, negotiate};
use str0m::{Candidate, Event, Rtc, RtcError};

const L_HOST: &str = "127.0.0.1:10001";
const R_BASE: &str = "127.0.0.1:10002";
const R_MAPPED: &str = "127.0.0.2:40000";

fn build_rtc(now: std::time::Instant) -> Rtc {
    Rtc::builder()
        .set_stats_interval(Some(Duration::from_millis(500)))
        .build(now)
}

#[test]
fn srflx_only_advertised_connects_through_nat() -> Result<(), RtcError> {
    let now = std::time::Instant::now();
    let mut sim = Sim::new(now, build_rtc(now), build_rtc(now));
    sim.nat.push((addr(R_BASE), addr(R_MAPPED)));
    sim.l_to_r.latency = Duration::from_millis(10);
    sim.r_to_l.latency = Duration::from_millis(10);

    sim.l
        .rtc
        .add_local_candidate(Candidate::host(addr(L_HOST), "udp").unwrap())
        .expect("L host accepted");
    let srflx = Candidate::server_reflexive(addr(R_MAPPED), addr(R_BASE), "udp").unwrap();
    sim.r
        .rtc
        .add_local_candidate(srflx)
        .expect("R srflx accepted");

    let (cid, offer_sdp, answer_sdp) =
        negotiate(&mut sim.l.rtc, &mut sim.r.rtc, |api| api.add_channel("control".into()))?;
    println!("--- offer ---\n{offer_sdp}--- answer ---\n{answer_sdp}---");
    assert!(answer_sdp.contains("typ srflx"), "answer must carry the srflx candidate");
    assert!(!answer_sdp.contains("typ host"), "answer must not carry a host candidate");
    assert!(answer_sdp.contains("127.0.0.2 40000"), "srflx address in answer");

    // Needed so R's agent accepts STUN requests arriving at BASE (see header comment).
    // Not signalled: the answer is already out.
    sim.r
        .rtc
        .add_local_candidate(Candidate::host(addr(R_BASE), "udp").unwrap())
        .expect("R local host (base) accepted");

    let t_conn = sim
        .run_until(Duration::from_secs(10), Duration::from_millis(5), |s| both_connected(s))?
        .expect("ICE+DTLS connect within 10s");
    println!("MEASURE connect_time={t_conn:?}");

    let open = sim.run_until(Duration::from_secs(5), Duration::from_millis(5), |s| {
        s.l.has_event(|e| matches!(e, Event::ChannelOpen(id, _) if *id == cid))
            && s.r.has_event(|e| matches!(e, Event::ChannelOpen(..)))
    })?;
    assert!(open.is_some(), "data channel opens on both sides");

    let ok = sim.l.rtc.channel(cid).expect("L channel").write(false, b"hello-srflx")?;
    assert!(ok);
    let got = sim.run_until(Duration::from_secs(5), Duration::from_millis(5), |s| {
        s.r.has_event(|e| matches!(e, Event::ChannelData(d) if d.data == b"hello-srflx"))
    })?;
    assert!(got.is_some(), "R receives data over the srflx path");

    let r_cid = sim
        .r
        .events
        .iter()
        .find_map(|(_, e)| match e {
            Event::ChannelOpen(id, _) => Some(*id),
            _ => None,
        })
        .unwrap();
    let ok = sim.r.rtc.channel(r_cid).expect("R channel").write(false, b"reply")?;
    assert!(ok);
    let got = sim.run_until(Duration::from_secs(5), Duration::from_millis(5), |s| {
        s.l.has_event(|e| matches!(e, Event::ChannelData(d) if d.data == b"reply"))
    })?;
    assert!(got.is_some(), "L receives R's reply");

    // Let at least one stats interval pass after nomination.
    sim.run_for(Duration::from_secs(1))?;

    // Direct observation: where each side actually sends.
    println!("OBSERVE L last_tx={:?}", sim.l.last_tx);
    println!("OBSERVE R last_tx={:?}", sim.r.last_tx);
    assert_eq!(sim.l.last_tx.unwrap().1, addr(R_MAPPED), "L sends to the mapped address");
    assert_eq!(sim.r.last_tx.unwrap().0, addr(R_BASE), "R sends from its base socket");

    // Stats observation of the selected pair.
    let pair = |peer: &common::Peer| {
        peer.events.iter().rev().find_map(|(_, e)| match e {
            Event::PeerStats(s) => s.selected_candidate_pair.clone(),
            _ => None,
        })
    };
    let l_pair = pair(&sim.l).expect("L PeerStats with selected pair");
    let r_pair = pair(&sim.r).expect("R PeerStats with selected pair");
    println!(
        "OBSERVE L selected local={} remote={}",
        l_pair.local.addr, l_pair.remote.addr
    );
    println!(
        "OBSERVE R selected local={} remote={}",
        r_pair.local.addr, r_pair.remote.addr
    );
    assert_eq!(l_pair.local.addr, addr(L_HOST));
    assert_eq!(l_pair.remote.addr, addr(R_MAPPED), "L's selected remote is the srflx");
    assert_eq!(r_pair.local.addr, addr(R_BASE), "R reports the base as local (send socket)");
    assert_eq!(r_pair.remote.addr, addr(L_HOST));

    println!(
        "MEASURE l_to_r={:?} r_to_l={:?}",
        sim.l_to_r.stats, sim.r_to_l.stats
    );
    Ok(())
}

/// Same setup but WITHOUT the local host candidate for BASE on R.
/// Expected from source (agent.rs `stun_server_handle_request`): R discards L's checks
/// ("Discarding STUN request on unknown interface"), L (controlling) never gets a
/// successful pair, so nothing is nominated and the connection does not come up.
/// This documents that the host app must keep a host candidate for the socket that
/// the UPnP mapping points at, even if it chooses not to signal it.
#[test]
fn srflx_without_local_host_base_does_not_connect() -> Result<(), RtcError> {
    let now = std::time::Instant::now();
    let mut sim = Sim::new(now, build_rtc(now), build_rtc(now));
    sim.nat.push((addr(R_BASE), addr(R_MAPPED)));

    sim.l
        .rtc
        .add_local_candidate(Candidate::host(addr(L_HOST), "udp").unwrap())
        .expect("L host accepted");
    sim.r
        .rtc
        .add_local_candidate(
            Candidate::server_reflexive(addr(R_MAPPED), addr(R_BASE), "udp").unwrap(),
        )
        .expect("R srflx accepted");

    let _ = negotiate(&mut sim.l.rtc, &mut sim.r.rtc, |api| api.add_channel("control".into()))?;

    let t = sim.run_until(Duration::from_secs(10), Duration::from_millis(5), |s| both_connected(s))?;
    println!(
        "MEASURE connected_within_10s={} l_to_r={:?} r_to_l={:?}",
        t.is_some(),
        sim.l_to_r.stats,
        sim.r_to_l.stats
    );
    assert!(t.is_none(), "expected no connection without a local host candidate at BASE");
    Ok(())
}
```

`spikes/webrtc-str0m/tests/ice_restart.rs`:

```rust
// Spike only (throwaway).
//! Q2. Viewer (L) changes IP. Can we recover with an ICE restart on the same Rtc and
//! keep using the same data channel?
//!
//! Flow: connect, exchange data, then L's old address 127.0.0.1:10001 disappears
//! (network drops everything to/from it). Wait for L to report ICE Disconnected
//! (measures detection time), then L does `sdp_api().ice_restart(false)`, renegotiates,
//! adds a host candidate on 127.0.0.1:10011 and trickles it to R.
//!
//! Source facts (str0m 0.23.1 / is 0.11.0):
//! - change/sdp.rs `SdpApi::ice_restart(keep_local_candidates)`; with `false` the offer
//!   carries no candidates (sdp.rs `AsSdpParams::new`) and the local candidates are
//!   cleared when the answer is accepted (sdp.rs `update_ice` -> agent.rs `ice_restart`).
//!   So the new local candidate must be added AFTER `accept_answer`, then trickled.
//! - is agent.rs `add_remote_candidate` rejects a candidate whose ufrag differs from the
//!   current remote credentials; `add_local_candidate` stamps the new local ufrag, and
//!   `Candidate::to_sdp_string` includes it, so the trickled string carries the new ufrag.
mod common;

use std::time::Duration;

use common::{Sim, addr, both_connected, negotiate};
use str0m::change::{SdpAnswer, SdpOffer};
use str0m::{Candidate, Event, IceConnectionState, Rtc, RtcError};

const L_OLD: &str = "127.0.0.1:10001";
const L_NEW: &str = "127.0.0.1:10011";
const R_HOST: &str = "127.0.0.1:10002";

#[test]
fn ice_restart_after_viewer_ip_change_keeps_channel() -> Result<(), RtcError> {
    let now = std::time::Instant::now();
    let mut sim = Sim::new(now, Rtc::builder().build(now), Rtc::builder().build(now));
    sim.l_to_r.latency = Duration::from_millis(20);
    sim.r_to_l.latency = Duration::from_millis(20);

    sim.l
        .rtc
        .add_local_candidate(Candidate::host(addr(L_OLD), "udp").unwrap())
        .expect("L host");
    sim.r
        .rtc
        .add_local_candidate(Candidate::host(addr(R_HOST), "udp").unwrap())
        .expect("R host");

    let (cid, _, _) =
        negotiate(&mut sim.l.rtc, &mut sim.r.rtc, |api| api.add_channel("control".into()))?;

    sim.run_until(Duration::from_secs(10), Duration::from_millis(5), |s| {
        both_connected(s)
            && s.l.has_event(|e| matches!(e, Event::ChannelOpen(id, _) if *id == cid))
            && s.r.has_event(|e| matches!(e, Event::ChannelOpen(..)))
    })?
    .expect("initial connect + channel open");

    let r_cid = sim
        .r
        .events
        .iter()
        .find_map(|(_, e)| match e {
            Event::ChannelOpen(id, _) => Some(*id),
            _ => None,
        })
        .unwrap();

    assert!(sim.l.rtc.channel(cid).unwrap().write(false, b"before")?);
    sim.run_until(Duration::from_secs(5), Duration::from_millis(5), |s| {
        s.r.has_event(|e| matches!(e, Event::ChannelData(d) if d.data == b"before"))
    })?
    .expect("data before IP change");

    // --- IP change: old address vanishes ---
    let t_change = sim.elapsed();
    sim.blocked.push(addr(L_OLD));
    let n_events_l = sim.l.events.len();

    let detect = sim.run_until(Duration::from_secs(60), Duration::from_millis(10), |s| {
        s.l.events[n_events_l..].iter().any(|(_, e)| {
            matches!(e, Event::IceConnectionStateChange(IceConnectionState::Disconnected))
        })
    })?;
    println!("MEASURE ice_disconnect_detect_after_ip_change={detect:?}");
    println!(
        "OBSERVE after outage: l.is_alive={} l.is_connected={} r.is_connected={}",
        sim.l.rtc.is_alive(),
        sim.l.rtc.is_connected(),
        sim.r.rtc.is_connected()
    );

    // --- ICE restart from L on the SAME Rtc ---
    let t_restart = sim.elapsed();
    let (offer, pending) = {
        let mut api = sim.l.rtc.sdp_api();
        let creds = api.ice_restart(false);
        println!("OBSERVE new L ufrag={}", creds.ufrag);
        api.apply().expect("ice restart needs negotiation")
    };
    let offer_sdp = offer.to_sdp_string();
    assert!(
        !offer_sdp.contains("a=candidate"),
        "keep_local_candidates=false: restart offer carries no candidates"
    );
    let answer = sim
        .r
        .rtc
        .sdp_api()
        .accept_offer(SdpOffer::from_sdp_string(&offer_sdp)?)?;
    let answer_sdp = answer.to_sdp_string();
    sim.l
        .rtc
        .sdp_api()
        .accept_answer(pending, SdpAnswer::from_sdp_string(&answer_sdp)?)?;

    // New interface on L, trickled to R as a string (signaling).
    let cand = sim
        .l
        .rtc
        .add_local_candidate(Candidate::host(addr(L_NEW), "udp").unwrap())
        .expect("L new host")
        .clone();
    let trickle = cand.to_sdp_string();
    println!("OBSERVE trickled candidate: {trickle}");
    sim.r
        .rtc
        .add_remote_candidate(Candidate::from_sdp_string(&trickle).unwrap());

    let t_ice = sim
        .run_until(Duration::from_secs(10), Duration::from_millis(1), |s| both_connected(s))?
        .expect("reconnect within 10s after restart");

    assert!(sim.l.rtc.channel(cid).is_some(), "same ChannelId still valid on L");
    assert!(sim.l.rtc.channel(cid).unwrap().write(false, b"after-restart")?);
    let t_data = sim
        .run_until(Duration::from_secs(10), Duration::from_millis(1), |s| {
            s.r.has_event(
                |e| matches!(e, Event::ChannelData(d) if d.id == r_cid && d.data == b"after-restart"),
            )
        })?
        .expect("data after restart on same channel");

    assert!(sim.r.rtc.channel(r_cid).unwrap().write(false, b"ack")?);
    sim.run_until(Duration::from_secs(5), Duration::from_millis(1), |s| {
        s.l.has_event(|e| matches!(e, Event::ChannelData(d) if d.id == cid && d.data == b"ack"))
    })?
    .expect("reverse data after restart");

    println!("OBSERVE L last_tx={:?} R last_tx={:?}", sim.l.last_tx, sim.r.last_tx);
    assert_eq!(sim.l.last_tx.unwrap().0, addr(L_NEW), "L now sends from the new address");
    assert_eq!(sim.r.last_tx.unwrap().1, addr(L_NEW), "R now sends to the new address");
    assert!(
        !sim.l.has_event(|e| matches!(e, Event::ChannelClose(_))),
        "channel never closed"
    );

    println!(
        "MEASURE ip_change_at={t_change:?} restart_at={t_restart:?} \
         restart_to_ice_connected={t_ice:?} ice_connected_to_first_data={t_data:?}"
    );
    Ok(())
}
```

`spikes/webrtc-str0m/tests/h264.rs`:

```rust
// Spike only (throwaway).
//! Q3. H.264 over str0m's frame-level API, and keyframe requests.
//!
//! Source facts (str0m 0.23.1):
//! - packet/h264.rs `H264Packetizer::packetize`: splits Annex-B input on 3/4-byte start
//!   codes. SPS/PPS are held and sent as one STAP-A before the next NAL. NALs larger
//!   than the MTU become FU-A fragments.
//! - packet/h264.rs `H264Depacketizer::depacketize` (is_avc=false, the default): every
//!   NAL (single, from STAP-A, or reassembled FU-A with header `ref_idc|type`) is written
//!   as `00 00 00 01` + NAL. So input written with 4-byte start codes and NAL header
//!   forbidden bit 0 comes out byte-identical in `MediaData::data`.
//! - Keyframe request direction: the RECEIVER calls `rtc.writer(mid)` and then
//!   `Writer::request_keyframe(rid, KeyframeRequestKind::Pli)` (media/writer.rs). It looks
//!   up the *receive* stream (`stream_rx_by_midrid`) and fails with NoReceiverSource if
//!   no RTP has arrived yet. The SENDER gets `Event::KeyframeRequest` (lib.rs Event).
//!   Same pattern in upstream tests/keyframe-requests.rs.
//! - Payload bytes must not contain `00 00 01`, otherwise the packetizer would split
//!   there. Dummy payload bytes below are never 0.
mod common;

use std::time::Duration;

use common::{Sim, addr, both_connected, negotiate};
use str0m::format::{Codec, CodecExtra};
use str0m::media::{Direction, KeyframeRequestKind, MediaKind, MediaTime};
use str0m::{Candidate, Event, Rtc, RtcError};

fn nal(header: u8, len: usize, seed: u8) -> Vec<u8> {
    let mut v = Vec::with_capacity(4 + len);
    v.extend_from_slice(&[0, 0, 0, 1, header]);
    v.extend((0..len).map(|i| ((i as u32 + seed as u32) % 250 + 1) as u8));
    v
}

fn frames() -> Vec<Vec<u8>> {
    let mut out = vec![];
    // Keyframe: SPS (7), PPS (8), IDR (5) with 5000 byte payload -> FU-A.
    let mut idr = nal(0x67, 20, 1);
    idr.extend(nal(0x68, 4, 2));
    idr.extend(nal(0x65, 5000, 3));
    out.push(idr);
    // Delta frames: non-IDR slice (1), 3000 bytes -> FU-A.
    for i in 0..5u8 {
        out.push(nal(0x41, 3000, 10 + i));
    }
    out
}

#[test]
fn h264_frames_roundtrip_and_keyframe_request() -> Result<(), RtcError> {
    let now = std::time::Instant::now();
    let mut sim = Sim::new(now, Rtc::builder().build(now), Rtc::builder().build(now));
    sim.l_to_r.latency = Duration::from_millis(10);
    sim.r_to_l.latency = Duration::from_millis(10);
    sim.l
        .rtc
        .add_local_candidate(Candidate::host(addr("127.0.0.1:10001"), "udp").unwrap())
        .unwrap();
    sim.r
        .rtc
        .add_local_candidate(Candidate::host(addr("127.0.0.1:10002"), "udp").unwrap())
        .unwrap();

    let (mid, _, answer_sdp) = negotiate(&mut sim.l.rtc, &mut sim.r.rtc, |api| {
        api.add_media(MediaKind::Video, Direction::SendOnly, None, None, None)
    })?;
    println!("--- answer ---\n{answer_sdp}---");

    sim.run_until(Duration::from_secs(10), Duration::from_millis(5), |s| both_connected(s))?
        .expect("connect");

    let params = sim
        .l
        .rtc
        .codec_config()
        .find(|p| p.spec().codec == Codec::H264 && p.spec().format.packetization_mode == Some(1))
        .cloned()
        .expect("H264 packetization-mode=1 in codec config");
    let pt = params.pt();
    println!("OBSERVE H264 pt={pt:?} spec={:?}", params.spec());

    let sent = frames();
    for (i, f) in sent.iter().enumerate() {
        let wallclock = sim.now;
        let rtp_time = MediaTime::from_90khz(i as u64 * 3000);
        sim.l
            .rtc
            .writer(mid)
            .expect("L writer")
            .write(pt, wallclock, rtp_time, f.clone())?;
        sim.run_for(Duration::from_millis(33))?;
    }
    sim.run_for(Duration::from_millis(500))?;

    let received: Vec<_> = sim
        .r
        .events
        .iter()
        .filter_map(|(t, e)| match e {
            Event::MediaData(d) if d.mid == mid => Some((*t, d)),
            _ => None,
        })
        .collect();
    println!("MEASURE frames_sent={} frames_received={}", sent.len(), received.len());
    assert_eq!(received.len(), sent.len(), "one MediaData per written frame");
    for (i, ((t, d), f)) in received.iter().zip(sent.iter()).enumerate() {
        let key = matches!(d.codec_extra, CodecExtra::H264(e) if e.is_keyframe);
        println!(
            "OBSERVE frame {i} at {t:?}: len={} sent_len={} keyframe={key} contiguous={} seq={:?}",
            d.data.len(),
            f.len(),
            d.contiguous,
            d.seq_range
        );
        assert_eq!(&d.data[..], &f[..], "frame {i} byte-identical after FU-A/STAP-A");
        assert_eq!(key, i == 0, "only frame 0 is a keyframe");
    }
    // 5000 byte IDR must have been fragmented into several RTP packets.
    let first = &received[0].1;
    let n_pkts = (**first.seq_range.end() - **first.seq_range.start()) + 1;
    println!("MEASURE keyframe_rtp_packets={n_pkts}");
    assert!(n_pkts >= 3, "IDR spans multiple RTP packets (STAP-A + FU-A)");

    // --- keyframe request: receiver asks, sender is told ---
    let t_req = sim.elapsed();
    {
        let mut w = sim.r.rtc.writer(mid).expect("R writer (used for requests on recv side)");
        assert!(w.is_request_keyframe_possible(KeyframeRequestKind::Pli));
        w.request_keyframe(None, KeyframeRequestKind::Pli)?;
    }
    let dt = sim
        .run_until(Duration::from_secs(2), Duration::from_millis(1), |s| {
            s.l.has_event(|e| {
                matches!(e, Event::KeyframeRequest(k)
                    if k.mid == mid && k.kind == KeyframeRequestKind::Pli)
            })
        })?
        .expect("sender (L) receives Event::KeyframeRequest");
    println!("MEASURE keyframe_request_at={t_req:?} delivered_after={dt:?}");
    assert!(
        !sim.r.has_event(|e| matches!(e, Event::KeyframeRequest(_))),
        "receiver does not get its own request"
    );
    Ok(())
}
```

`spikes/webrtc-str0m/tests/data_channel.rs`:

```rust
// Spike only (throwaway).
//! Q4. Reliable-ordered vs unordered/MaxRetransmits{0} data channels under loss.
//!
//! After both channels are open, every 5th packet is dropped in each direction
//! (deterministic). L sends 200 messages on each channel, one pair every 5 ms.
//! Expect: reliable gets all 200 in order; unreliable gets some but not all.
//!
//! Source facts (str0m 0.23.1): `ChannelConfig { label, ordered, reliability,
//! negotiated, protocol }` and `Reliability::{Reliable, MaxPacketLifetime{lifetime},
//! MaxRetransmits{retransmits}}` in sctp/mod.rs, re-exported from `str0m::channel`.
//! `SdpApi::add_channel_with_config` in change/sdp.rs. `Channel::write(binary, buf)
//! -> Result<bool>` (false = not accepted, buffer full) in channel.rs.
mod common;

use std::time::Duration;

use common::{Sim, addr, both_connected, negotiate};
use str0m::channel::{ChannelConfig, ChannelId, Reliability};
use str0m::{Candidate, Event, Rtc, RtcError};

const N: usize = 200;

fn msg(prefix: u8, i: usize) -> Vec<u8> {
    let mut v = format!("{}{:03}", prefix as char, i).into_bytes();
    v.resize(300, b'.');
    v
}

fn seq_of(data: &[u8]) -> usize {
    std::str::from_utf8(&data[1..4]).unwrap().parse().unwrap()
}

#[test]
fn reliable_vs_unreliable_under_loss() -> Result<(), RtcError> {
    let now = std::time::Instant::now();
    let mut sim = Sim::new(now, Rtc::builder().build(now), Rtc::builder().build(now));
    sim.l_to_r.latency = Duration::from_millis(15);
    sim.r_to_l.latency = Duration::from_millis(15);
    sim.l
        .rtc
        .add_local_candidate(Candidate::host(addr("127.0.0.1:10001"), "udp").unwrap())
        .unwrap();
    sim.r
        .rtc
        .add_local_candidate(Candidate::host(addr("127.0.0.1:10002"), "udp").unwrap())
        .unwrap();

    let ((rel, unrel), _, _) = negotiate(&mut sim.l.rtc, &mut sim.r.rtc, |api| {
        let rel = api.add_channel_with_config(ChannelConfig {
            label: "reliable".into(),
            ..Default::default()
        });
        let unrel = api.add_channel_with_config(ChannelConfig {
            label: "unreliable".into(),
            ordered: false,
            reliability: Reliability::MaxRetransmits { retransmits: 0 },
            ..Default::default()
        });
        (rel, unrel)
    })?;

    let r_id = |s: &Sim, label: &str| -> Option<ChannelId> {
        s.r.events.iter().find_map(|(_, e)| match e {
            Event::ChannelOpen(id, l) if l == label => Some(*id),
            _ => None,
        })
    };

    sim.run_until(Duration::from_secs(10), Duration::from_millis(5), |s| {
        both_connected(s)
            && s.l.has_event(|e| matches!(e, Event::ChannelOpen(id, _) if *id == rel))
            && s.l.has_event(|e| matches!(e, Event::ChannelOpen(id, _) if *id == unrel))
            && r_id(s, "reliable").is_some()
            && r_id(s, "unreliable").is_some()
    })?
    .expect("both channels open");
    let r_rel = r_id(&sim, "reliable").unwrap();
    let r_unrel = r_id(&sim, "unreliable").unwrap();

    let cfg = sim.r.rtc.channel(r_unrel).unwrap().config().cloned();
    println!("OBSERVE R view of unreliable channel config: {cfg:?}");

    // Loss starts now.
    sim.l_to_r.drop_every_nth = Some(5);
    sim.r_to_l.drop_every_nth = Some(5);
    let t0 = sim.elapsed();

    for i in 0..N {
        assert!(sim.l.rtc.channel(rel).unwrap().write(true, &msg(b'r', i))?, "rel write {i}");
        assert!(sim.l.rtc.channel(unrel).unwrap().write(true, &msg(b'u', i))?, "unrel write {i}");
        sim.run_for(Duration::from_millis(5))?;
    }

    let count = |s: &Sim, id: ChannelId| {
        s.r.events
            .iter()
            .filter(|(_, e)| matches!(e, Event::ChannelData(d) if d.id == id))
            .count()
    };
    let done = sim.run_until(Duration::from_secs(60), Duration::from_millis(10), |s| {
        count(s, r_rel) >= N
    })?;
    // Give unreliable stragglers a moment.
    sim.run_for(Duration::from_secs(1))?;

    let rel_seq: Vec<usize> = sim
        .r
        .events
        .iter()
        .filter_map(|(_, e)| match e {
            Event::ChannelData(d) if d.id == r_rel => Some(seq_of(&d.data)),
            _ => None,
        })
        .collect();
    let unrel_seq: Vec<usize> = sim
        .r
        .events
        .iter()
        .filter_map(|(_, e)| match e {
            Event::ChannelData(d) if d.id == r_unrel => Some(seq_of(&d.data)),
            _ => None,
        })
        .collect();
    let last_rel_at = sim
        .r
        .events
        .iter()
        .filter(|(_, e)| matches!(e, Event::ChannelData(d) if d.id == r_rel))
        .map(|(t, _)| *t)
        .last();

    println!(
        "MEASURE reliable_received={} unreliable_received={} all_reliable_after={:?} \
         last_reliable_at={:?} (loss start {:?})",
        rel_seq.len(),
        unrel_seq.len(),
        done,
        last_rel_at,
        t0
    );
    println!("MEASURE l_to_r={:?} r_to_l={:?}", sim.l_to_r.stats, sim.r_to_l.stats);
    let mut missing: Vec<usize> = (0..N).filter(|i| !unrel_seq.contains(i)).collect();
    missing.truncate(30);
    println!("OBSERVE first missing unreliable seqs: {missing:?}");

    assert_eq!(rel_seq, (0..N).collect::<Vec<_>>(), "reliable: all 200, in order");
    assert!(unrel_seq.len() < N, "unreliable must lose something under 20% loss");
    assert!(!unrel_seq.is_empty(), "unreliable must deliver something");
    let mut dedup = unrel_seq.clone();
    dedup.sort();
    dedup.dedup();
    assert_eq!(dedup.len(), unrel_seq.len(), "no duplicates on unreliable");
    assert!(sim.l.rtc.is_connected() && sim.r.rtc.is_connected());
    Ok(())
}
```

`spikes/webrtc-str0m/tests/bwe.rs`:

```rust
// Spike only (throwaway).
//! Q5. Does str0m's send-side BWE (TWCC) track a bottleneck and recover when it opens?
//!
//! Setup: L sends H.264 at a rate that follows the latest estimate (like a real encoder
//! would), with `set_desired_bitrate(5 Mbps)` so str0m probes/pads above the media rate.
//! Phase 1 (20 s): L->R bottleneck 1 Mbps, 20 ms latency, 50 KB queue (~400 ms).
//! Phase 2 (20 s): bottleneck raised to 5 Mbps.
//!
//! Source facts (str0m 0.23.1):
//! - `RtcConfig::enable_bwe(Option<Bitrate>)` in config.rs (Some = on, initial estimate).
//! - `Bitrate::{bps,kbps,mbps}` const fns, `as_u64`, `Display` in str0m-proto bandwidth.rs;
//!   re-exported as `str0m::bwe::Bitrate`.
//! - `rtc.bwe().set_desired_bitrate(..)` in bwe/api.rs; estimates arrive as
//!   `Event::EgressBitrateEstimate(BweKind::Twcc(Bitrate))`.
//! - TWCC feedback is enabled automatically in SDP mode when an m-line has
//!   `a=rtcp-fb:* transport-cc` and the transport-cc header extension (change/sdp.rs,
//!   `session.enable_twcc_feedback()`); upstream tests/bwe/common.rs needs the explicit
//!   call only because it uses the direct API.
//! - Pacer runs at 2x the estimate (session.rs `PacerImpl::leaky_bucket(rate * 2.0)`),
//!   padding/probing needs a padding queue (RTX), which SDP negotiation sets up.
mod common;

use std::time::{Duration, Instant};

use common::{Sim, addr, both_connected, negotiate};
use str0m::bwe::{Bitrate, BweKind};
use str0m::format::Codec;
use str0m::media::{Direction, MediaKind, MediaTime, Mid, Pt};
use str0m::{Candidate, Event, Rtc, RtcError};

const FPS: u64 = 30;

fn last_estimate(sim: &Sim) -> Option<(Duration, Bitrate)> {
    sim.l.events.iter().rev().find_map(|(t, e)| match e {
        Event::EgressBitrateEstimate(BweKind::Twcc(b)) => Some((*t, *b)),
        _ => None,
    })
}

fn estimates_between(sim: &Sim, from: Duration, to: Duration) -> Vec<(Duration, Bitrate)> {
    sim.l
        .events
        .iter()
        .filter_map(|(t, e)| match e {
            Event::EgressBitrateEstimate(BweKind::Twcc(b)) if *t >= from && *t < to => {
                Some((*t, *b))
            }
            _ => None,
        })
        .collect()
}

fn delta_frame(len: usize, seed: u64) -> Vec<u8> {
    let mut v = Vec::with_capacity(5 + len);
    v.extend_from_slice(&[0, 0, 0, 1, 0x41]);
    v.extend((0..len).map(|i| ((i as u64 + seed) % 250 + 1) as u8));
    v
}

struct Sender {
    mid: Mid,
    pt: Pt,
    frame_no: u64,
    sent_bytes: u64,
}

impl Sender {
    fn run_phase(&mut self, sim: &mut Sim, dur: Duration, label: &str) -> Result<(), RtcError> {
        let end = sim.now + dur;
        let mut next_print = sim.now;
        while sim.now < end {
            let target = last_estimate(sim)
                .map(|(_, b)| b)
                .unwrap_or(Bitrate::kbps(300))
                .clamp(Bitrate::kbps(200), Bitrate::mbps(4));
            let frame_len = (target.as_u64() / 8 / FPS) as usize;
            let frame = delta_frame(frame_len.max(200), self.frame_no);
            self.sent_bytes += frame.len() as u64;
            let wallclock: Instant = sim.now;
            sim.l.rtc.writer(self.mid).expect("writer").write(
                self.pt,
                wallclock,
                MediaTime::from_90khz(self.frame_no * (90_000 / FPS)),
                frame,
            )?;
            self.frame_no += 1;
            sim.run_for(Duration::from_micros(1_000_000 / FPS))?;

            if sim.now >= next_print {
                let est = last_estimate(sim).map(|(_, b)| b.to_string());
                println!(
                    "TIMELINE {label} t={:>6.2}s est={:<12} media_target={} qdelay={:>4}ms l_to_r={:?}",
                    sim.elapsed().as_secs_f64(),
                    est.unwrap_or_else(|| "-".into()),
                    target,
                    sim.l_to_r.queue_delay(sim.now).as_millis(),
                    sim.l_to_r.stats
                );
                next_print += Duration::from_secs(1);
            }
        }
        Ok(())
    }
}

#[test]
fn bwe_tracks_bottleneck_then_recovers() -> Result<(), RtcError> {
    let now = Instant::now();
    let l_rtc = Rtc::builder().enable_bwe(Some(Bitrate::kbps(300))).build(now);
    let r_rtc = Rtc::builder().build(now);
    let mut sim = Sim::new(now, l_rtc, r_rtc);
    sim.l.verbose = true;
    sim.r.verbose = false;
    sim.l_to_r.latency = Duration::from_millis(20);
    sim.r_to_l.latency = Duration::from_millis(20);
    sim.l
        .rtc
        .add_local_candidate(Candidate::host(addr("127.0.0.1:10001"), "udp").unwrap())
        .unwrap();
    sim.r
        .rtc
        .add_local_candidate(Candidate::host(addr("127.0.0.1:10002"), "udp").unwrap())
        .unwrap();

    let (mid, offer_sdp, _) = negotiate(&mut sim.l.rtc, &mut sim.r.rtc, |api| {
        api.add_media(MediaKind::Video, Direction::SendOnly, None, None, None)
    })?;
    assert!(offer_sdp.contains("transport-cc"), "TWCC negotiated in SDP");

    sim.run_until(Duration::from_secs(10), Duration::from_millis(5), |s| both_connected(s))?
        .expect("connect");

    let pt = sim
        .l
        .rtc
        .codec_config()
        .find(|p| p.spec().codec == Codec::H264 && p.spec().format.packetization_mode == Some(1))
        .map(|p| p.pt())
        .expect("H264 pt");
    sim.l.rtc.bwe().set_desired_bitrate(Bitrate::mbps(5));

    // Phase 1: 1 Mbps bottleneck.
    sim.l_to_r.rate_bps = Some(1_000_000);
    sim.l_to_r.queue_limit_bytes = 50_000;
    let mut tx = Sender {
        mid,
        pt,
        frame_no: 0,
        sent_bytes: 0,
    };
    let p1_start = sim.elapsed();
    tx.run_phase(&mut sim, Duration::from_secs(20), "1Mbps")?;
    let p1_end = sim.elapsed();

    // Phase 2: 5 Mbps.
    sim.l_to_r.rate_bps = Some(5_000_000);
    sim.l_to_r.queue_limit_bytes = 250_000;
    tx.run_phase(&mut sim, Duration::from_secs(20), "5Mbps")?;
    let p2_end = sim.elapsed();

    let p1_tail = estimates_between(&sim, p1_end - Duration::from_secs(5), p1_end);
    let p1_all = estimates_between(&sim, p1_start, p1_end);
    let p2_all = estimates_between(&sim, p1_end, p2_end);
    let p1_last = p1_all.last().map(|(_, b)| *b).expect("estimates in phase 1");
    let p1_tail_max = p1_tail.iter().map(|(_, b)| b.as_u64()).max();
    let p1_peak = p1_all.iter().map(|(_, b)| b.as_u64()).max().unwrap();
    let p2_max = p2_all.iter().map(|(_, b)| b.as_u64()).max().expect("estimates in phase 2");
    let p2_last = p2_all.last().map(|(_, b)| *b).unwrap();

    println!(
        "MEASURE phase1: n_est={} peak={} last={} tail5s_max={:?}",
        p1_all.len(),
        Bitrate::bps(p1_peak),
        p1_last,
        p1_tail_max.map(Bitrate::bps)
    );
    println!(
        "MEASURE phase2: n_est={} max={} last={} first_above_1.5M_at={:?}",
        p2_all.len(),
        Bitrate::bps(p2_max),
        p2_last,
        p2_all
            .iter()
            .find(|(_, b)| b.as_u64() > 1_500_000)
            .map(|(t, _)| *t - p1_end)
    );
    println!(
        "MEASURE media_bytes_written={} l_to_r={:?}",
        tx.sent_bytes, sim.l_to_r.stats
    );

    assert!(
        p1_last.as_u64() <= 1_200_000,
        "phase 1 estimate should settle at or below ~1.2 Mbps, got {p1_last}"
    );
    assert!(
        p2_max > 1_500_000,
        "phase 2 estimate should rise above 1.5 Mbps once capacity is 5 Mbps, max {}",
        Bitrate::bps(p2_max)
    );
    Ok(())
}
```


---

## 부록 E. `spikes/webrtc-rs`

`spikes/webrtc-rs/Cargo.toml`:

```toml
[package]
name = "spike-webrtc-rs"
version = "0.1.0"
edition = "2024"
publish = false

[lib]
path = "src/lib.rs"

[dev-dependencies]
webrtc = "=0.21.0"
# `rtc` is the sans-I/O core that `webrtc` wraps. Several types the async API takes
# (RTCOfferOptions, MulticastDnsMode, MediaStreamTrack, codec structs, interceptors,
# rtp/rtcp packets) are only reachable through it, so it is pinned to the same version.
rtc = "=0.21.0"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time", "net"] }
async-trait = "0.1"
bytes = "1"
```

`spikes/webrtc-rs/src/lib.rs`:

```rust
// Spike only (throwaway).
//! Spike package: all code lives in tests/.
```

`spikes/webrtc-rs/tests/common/mod.rs`:

```rust
// Spike only (throwaway).
//! Shared harness for the webrtc 0.21.0 spike tests.
//!
//! - `build_peer`: a loopback peer with mDNS disabled and an event handler that turns callbacks
//!   into channels (`Events`).
//! - `make_offer` / `make_answer` / `negotiate`: non-trickle signaling (wait for gathering to
//!   complete, then hand over the full SDP).
//! - SDP helpers that remove or replace `a=candidate:` lines. The async `webrtc` API has no
//!   `add_local_candidate` (only the sans-I/O `rtc` core has one, and the driver owns it), so the
//!   only way to advertise an extra local address is to edit the SDP we send.
//! - `Forwarder`: a small NAT-like UDP relay (one "public" socket, one "inside" socket per
//!   outside peer) with optional loss and a rate-limited drop-tail queue.
#![allow(dead_code)]

use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket as StdUdpSocket};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rtc::ice::mdns::MulticastDnsMode; // not re-exported by `webrtc`
use rtc::peer_connection::configuration::RTCOfferOptions; // not re-exported by `webrtc`
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use webrtc::data_channel::{DataChannel, DataChannelEvent};
use webrtc::media_stream::track_remote::TrackRemote;
use webrtc::peer_connection::{
    MediaEngine, PeerConnection, PeerConnectionBuilder, PeerConnectionEventHandler,
    RTCIceGatheringState, RTCPeerConnectionState, RTCSessionDescription, Registry, SettingEngine,
    SettingEngineBuilder,
};
use webrtc::runtime::TokioRuntime;

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;
pub type TestResult<T = ()> = Result<T, BoxError>;

pub const GATHER_TIMEOUT: Duration = Duration::from_secs(5);
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

// ───────────────────────────── peer + events ─────────────────────────────

/// Callback outputs of one peer, as channels.
pub struct Events {
    pub name: &'static str,
    pub gathered: mpsc::UnboundedReceiver<()>,
    pub state: watch::Receiver<RTCPeerConnectionState>,
    pub data_channels: mpsc::UnboundedReceiver<Arc<dyn DataChannel>>,
    pub tracks: mpsc::UnboundedReceiver<Arc<dyn TrackRemote>>,
    /// Every connection state change with the time it was observed.
    pub history: Arc<Mutex<Vec<(Instant, RTCPeerConnectionState)>>>,
}

struct Handler {
    name: &'static str,
    gathered: mpsc::UnboundedSender<()>,
    state: watch::Sender<RTCPeerConnectionState>,
    data_channels: mpsc::UnboundedSender<Arc<dyn DataChannel>>,
    tracks: mpsc::UnboundedSender<Arc<dyn TrackRemote>>,
    history: Arc<Mutex<Vec<(Instant, RTCPeerConnectionState)>>>,
}

#[async_trait::async_trait]
impl PeerConnectionEventHandler for Handler {
    async fn on_ice_gathering_state_change(&self, state: RTCIceGatheringState) {
        if state == RTCIceGatheringState::Complete {
            let _ = self.gathered.send(());
        }
    }
    async fn on_connection_state_change(&self, state: RTCPeerConnectionState) {
        println!("[{}] connection state: {state}", self.name);
        self.history.lock().unwrap().push((Instant::now(), state));
        let _ = self.state.send(state);
    }
    async fn on_data_channel(&self, dc: Arc<dyn DataChannel>) {
        let _ = self.data_channels.send(dc);
    }
    async fn on_track(&self, track: Arc<dyn TrackRemote>) {
        // Must not block: the driver awaits this callback.
        let _ = self.tracks.send(track);
    }
}

/// Setting engine with mDNS off. The default mode (QueryOnly) binds UDP 5353, and failing to
/// bind it is fatal for the connection; we never need `.local` names on loopback.
pub fn setting_engine() -> SettingEngineBuilder {
    SettingEngineBuilder::new().with_multicast_dns_mode(MulticastDnsMode::Disabled)
}

/// Builds a peer bound to exactly one UDP address (e.g. "127.0.0.1:0").
///
/// `media`: `(MediaEngine, Registry)` for media tests; `None` keeps the library defaults.
pub async fn build_peer(
    name: &'static str,
    udp_addr: &str,
    media: Option<(MediaEngine, Registry)>,
    setting_engine: SettingEngine,
) -> TestResult<(Arc<dyn PeerConnection>, Events)> {
    let (gtx, grx) = mpsc::unbounded_channel();
    let (stx, srx) = watch::channel(RTCPeerConnectionState::New);
    let (dtx, drx) = mpsc::unbounded_channel();
    let (ttx, trx) = mpsc::unbounded_channel();
    let history = Arc::new(Mutex::new(Vec::new()));
    let handler = Handler {
        name,
        gathered: gtx,
        state: stx,
        data_channels: dtx,
        tracks: ttx,
        history: Arc::clone(&history),
    };
    let mut builder = PeerConnectionBuilder::new()
        .with_setting_engine(setting_engine)
        .with_runtime(Arc::new(TokioRuntime))
        .with_handler(Arc::new(handler))
        .with_udp_addrs(vec![udp_addr.to_string()]);
    if let Some((media_engine, registry)) = media {
        builder = builder
            .with_media_engine(media_engine)
            .with_interceptor_registry(registry);
    }
    let pc = builder.build().await?;
    Ok((
        Arc::new(pc) as Arc<dyn PeerConnection>,
        Events {
            name,
            gathered: grx,
            state: srx,
            data_channels: drx,
            tracks: trx,
            history,
        },
    ))
}

/// Drops gathering-complete signals left over from an earlier negotiation.
fn drain_gathered(events: &mut Events) {
    while events.gathered.try_recv().is_ok() {}
}

/// Waits for gathering to complete. A renegotiation that does not restart ICE never gathers
/// again, so a timeout here is not an error: we print it and use what we have.
async fn wait_gathered(events: &mut Events) {
    let t = Instant::now();
    match tokio::time::timeout(GATHER_TIMEOUT, events.gathered.recv()).await {
        Ok(_) => println!("[{}] gathering complete in {:?}", events.name, t.elapsed()),
        Err(_) => println!(
            "[{}] no gathering-complete within {:?} (no new gathering?)",
            events.name, GATHER_TIMEOUT
        ),
    }
}

/// create_offer + set_local_description + wait for gathering; returns the full local offer.
pub async fn make_offer(
    pc: &Arc<dyn PeerConnection>,
    events: &mut Events,
    options: Option<RTCOfferOptions>,
) -> TestResult<RTCSessionDescription> {
    drain_gathered(events);
    let offer = pc.create_offer(options).await?;
    pc.set_local_description(offer).await?;
    wait_gathered(events).await;
    Ok(pc.local_description().await.ok_or("no local offer")?)
}

/// set_remote_description(offer) + create_answer + set_local_description + wait for gathering.
pub async fn make_answer(
    pc: &Arc<dyn PeerConnection>,
    events: &mut Events,
    offer: RTCSessionDescription,
) -> TestResult<RTCSessionDescription> {
    drain_gathered(events);
    pc.set_remote_description(offer).await?;
    let answer = pc.create_answer(None).await?;
    pc.set_local_description(answer).await?;
    wait_gathered(events).await;
    Ok(pc.local_description().await.ok_or("no local answer")?)
}

/// Plain non-trickle offer/answer with no SDP edits.
pub async fn negotiate(
    offerer: &Arc<dyn PeerConnection>,
    oe: &mut Events,
    answerer: &Arc<dyn PeerConnection>,
    ae: &mut Events,
    options: Option<RTCOfferOptions>,
) -> TestResult {
    let offer = make_offer(offerer, oe, options).await?;
    let answer = make_answer(answerer, ae, offer).await?;
    offerer.set_remote_description(answer).await?;
    Ok(())
}

/// Waits until the connection state is `Connected`.
pub async fn wait_connected(events: &mut Events, limit: Duration) -> TestResult {
    let name = events.name;
    tokio::time::timeout(
        limit,
        events
            .state
            .wait_for(|s| *s == RTCPeerConnectionState::Connected),
    )
    .await
    .map_err(|_| format!("[{name}] not connected within {limit:?}"))??;
    Ok(())
}

pub fn print_history(events: &Events, origin: Instant) {
    for (t, s) in events.history.lock().unwrap().iter() {
        let ms = t.saturating_duration_since(origin).as_millis();
        println!("[{}] +{ms} ms {s}", events.name);
    }
}

// ───────────────────────────── data channel ─────────────────────────────

/// One task polls a data channel (poll() consumes every event, so there must be exactly one
/// reader) and splits the events into an "opened" signal and a message stream.
pub struct DcReader {
    pub opened: Option<oneshot::Receiver<()>>,
    pub msgs: mpsc::UnboundedReceiver<Vec<u8>>,
}

pub fn spawn_dc_reader(dc: Arc<dyn DataChannel>) -> DcReader {
    let (otx, orx) = oneshot::channel();
    let (mtx, mrx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let mut otx = Some(otx);
        while let Some(ev) = dc.poll().await {
            match ev {
                DataChannelEvent::OnOpen => {
                    if let Some(tx) = otx.take() {
                        let _ = tx.send(());
                    }
                }
                DataChannelEvent::OnMessage(m) => {
                    // A channel handed to on_data_channel may already be open, so the first
                    // message also counts as "opened".
                    if let Some(tx) = otx.take() {
                        let _ = tx.send(());
                    }
                    let _ = mtx.send(m.data.to_vec());
                }
                DataChannelEvent::OnClose => break,
                _ => {}
            }
        }
    });
    DcReader {
        opened: Some(orx),
        msgs: mrx,
    }
}

impl DcReader {
    pub async fn wait_open(&mut self, limit: Duration) -> TestResult {
        if let Some(rx) = self.opened.take() {
            tokio::time::timeout(limit, rx)
                .await
                .map_err(|_| "data channel did not open")??;
        }
        Ok(())
    }

    pub async fn recv(&mut self, limit: Duration) -> TestResult<Vec<u8>> {
        Ok(tokio::time::timeout(limit, self.msgs.recv())
            .await
            .map_err(|_| "no data channel message in time")?
            .ok_or("data channel reader ended")?)
    }
}

// ───────────────────────────── SDP editing ─────────────────────────────

fn is_candidate_line(line: &str) -> bool {
    line.starts_with("a=candidate:")
}

/// Removes every `a=candidate:` line (keeps `a=end-of-candidates`).
pub fn strip_candidates(sdp: &str) -> String {
    replace_candidates(sdp, &[])
}

/// Removes every `a=candidate:` line and puts `new_lines` where the first one was.
/// webrtc-rs writes candidates only into the first m-section (BUNDLE), so that is where they go.
pub fn replace_candidates(sdp: &str, new_lines: &[String]) -> String {
    let mut out = Vec::new();
    let mut inserted = false;
    for line in sdp.split("\r\n") {
        if is_candidate_line(line) {
            if !inserted {
                out.extend(new_lines.iter().cloned());
                inserted = true;
            }
            continue;
        }
        out.push(line.to_string());
    }
    assert!(
        inserted || new_lines.is_empty(),
        "SDP had no candidate line to replace"
    );
    out.join("\r\n")
}

/// Host candidate addresses (UDP, component 1) found in an SDP.
pub fn host_candidates(sdp: &str) -> Vec<SocketAddr> {
    sdp.split("\r\n")
        .filter_map(|l| l.strip_prefix("a=candidate:"))
        .filter_map(|v| {
            // foundation component transport priority address port "typ" type ...
            let f: Vec<&str> = v.split_whitespace().collect();
            if f.len() >= 8 && f[1] == "1" && f[2].eq_ignore_ascii_case("udp") && f[7] == "host" {
                format!("{}:{}", f[4], f[5]).parse().ok()
            } else {
                None
            }
        })
        .collect()
}

/// `a=candidate:` line for a server-reflexive address (what a UPnP port mapping gives us).
/// Priority = (type pref 100 << 24) | (local pref 65535 << 8) | (256 - component 1), RFC 8445 §5.1.2.1.
pub fn srflx_candidate_line(public: SocketAddr, base: SocketAddr) -> String {
    let priority: u32 = (100u32 << 24) | (65535u32 << 8) | 255;
    format!(
        "a=candidate:9999 1 udp {priority} {} {} typ srflx raddr {} rport {}",
        public.ip(),
        public.port(),
        base.ip(),
        base.port()
    )
}

pub fn with_sdp(desc: &RTCSessionDescription, sdp: String) -> TestResult<RTCSessionDescription> {
    // Rebuild through the constructors: they re-parse, and RTCSessionDescription caches the
    // parsed form in a private field that a plain `.sdp = ...` edit would leave stale.
    use webrtc::peer_connection::RTCSdpType;
    Ok(match desc.sdp_type {
        RTCSdpType::Offer => RTCSessionDescription::offer(sdp)?,
        RTCSdpType::Answer => RTCSessionDescription::answer(sdp)?,
        other => return Err(format!("unexpected sdp type {other}").into()),
    })
}

/// Finds a free UDP port on `ip` by binding port 0 and releasing it.
pub fn free_udp_port(ip: &str) -> u16 {
    let s = StdUdpSocket::bind(format!("{ip}:0")).expect("bind probe socket");
    s.local_addr().unwrap().port()
}

// ───────────────────────────── UDP forwarder ─────────────────────────────

/// Queue limit of the rate-limited path: a packet that would wait longer than this is dropped
/// (drop-tail), like a router with a ~200 ms buffer.
pub const MAX_QUEUE_DELAY: Duration = Duration::from_millis(200);

/// NAT-like UDP relay in front of one `target` (the inside host).
///
/// Outside peer X sends to `public_addr` → forwarder relays from an inside socket dedicated to X
/// → target. Target replies to that inside socket → forwarder sends from `public_addr` back to X.
/// So X sees only `public_addr`, and the target sees one stable inside address per outside peer
/// (it learns that address as a peer-reflexive candidate).
///
/// Shaping (`drop_every_nth`, `rate_bps`) applies to the inbound direction (outside → target)
/// only; set the knobs at any time.
pub struct Forwarder {
    pub public_addr: SocketAddr,
    /// 0 = off. When n > 0, every n-th inbound datagram is dropped.
    pub drop_every_nth: Arc<AtomicU64>,
    /// 0 = unlimited. Otherwise inbound bits per second, drop-tail queue of MAX_QUEUE_DELAY.
    pub rate_bps: Arc<AtomicU64>,
    pub inbound_forwarded: Arc<AtomicU64>,
    pub inbound_dropped: Arc<AtomicU64>,
    pub inbound_bytes: Arc<AtomicU64>,
    pub outbound_forwarded: Arc<AtomicU64>,
    tasks: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl Drop for Forwarder {
    fn drop(&mut self) {
        for t in self.tasks.lock().unwrap().drain(..) {
            t.abort();
        }
    }
}

impl Forwarder {
    pub async fn start(public_bind: &str, target: SocketAddr) -> TestResult<Forwarder> {
        let public = Arc::new(UdpSocket::bind(public_bind).await?);
        let public_addr = public.local_addr()?;
        let f = Forwarder {
            public_addr,
            drop_every_nth: Arc::new(AtomicU64::new(0)),
            rate_bps: Arc::new(AtomicU64::new(0)),
            inbound_forwarded: Arc::new(AtomicU64::new(0)),
            inbound_dropped: Arc::new(AtomicU64::new(0)),
            inbound_bytes: Arc::new(AtomicU64::new(0)),
            outbound_forwarded: Arc::new(AtomicU64::new(0)),
            tasks: Arc::new(Mutex::new(Vec::new())),
        };

        // Rate-limited path: packets carry their departure deadline; deadlines are monotonic,
        // so one FIFO task that sleeps until each deadline is an exact serializer.
        let (shape_tx, mut shape_rx) =
            mpsc::unbounded_channel::<(tokio::time::Instant, Vec<u8>, Arc<UdpSocket>)>();
        let shaper = tokio::spawn(async move {
            while let Some((deadline, data, inside)) = shape_rx.recv().await {
                tokio::time::sleep_until(deadline).await;
                let _ = inside.send_to(&data, target).await;
            }
        });

        let drop_every_nth = Arc::clone(&f.drop_every_nth);
        let rate_bps = Arc::clone(&f.rate_bps);
        let in_fwd = Arc::clone(&f.inbound_forwarded);
        let in_drop = Arc::clone(&f.inbound_dropped);
        let in_bytes = Arc::clone(&f.inbound_bytes);
        let out_fwd = Arc::clone(&f.outbound_forwarded);
        let tasks = Arc::clone(&f.tasks);

        let main = tokio::spawn(async move {
            let mut mappings: HashMap<SocketAddr, Arc<UdpSocket>> = HashMap::new();
            let mut buf = vec![0u8; 65536];
            let mut nth_counter: u64 = 0;
            let mut next_free = tokio::time::Instant::now();
            loop {
                let Ok((n, from)) = public.recv_from(&mut buf).await else {
                    break;
                };
                let inside = match mappings.get(&from) {
                    Some(s) => Arc::clone(s),
                    None => {
                        let Ok(s) = UdpSocket::bind("127.0.0.1:0").await else {
                            continue;
                        };
                        let s = Arc::new(s);
                        println!(
                            "[forwarder] new mapping outside {from} <-> inside {}",
                            s.local_addr().unwrap()
                        );
                        // Reverse direction for this mapping.
                        let rs = Arc::clone(&s);
                        let rp = Arc::clone(&public);
                        let ro = Arc::clone(&out_fwd);
                        let rev = tokio::spawn(async move {
                            let mut rbuf = vec![0u8; 65536];
                            while let Ok((m, _)) = rs.recv_from(&mut rbuf).await {
                                if rp.send_to(&rbuf[..m], from).await.is_ok() {
                                    ro.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                        });
                        tasks.lock().unwrap().push(rev);
                        mappings.insert(from, Arc::clone(&s));
                        s
                    }
                };

                let nth = drop_every_nth.load(Ordering::Relaxed);
                if nth > 0 {
                    nth_counter += 1;
                    if nth_counter % nth == 0 {
                        in_drop.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                }

                let rate = rate_bps.load(Ordering::Relaxed);
                if rate == 0 {
                    if inside.send_to(&buf[..n], target).await.is_ok() {
                        in_fwd.fetch_add(1, Ordering::Relaxed);
                        in_bytes.fetch_add(n as u64, Ordering::Relaxed);
                    }
                    continue;
                }
                let now = tokio::time::Instant::now();
                let start = next_free.max(now);
                if start - now > MAX_QUEUE_DELAY {
                    in_drop.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
                // Count IP+UDP header (28 bytes) so the limit is on-the-wire-ish.
                let tx_time = Duration::from_secs_f64(((n + 28) * 8) as f64 / rate as f64);
                next_free = start + tx_time;
                in_fwd.fetch_add(1, Ordering::Relaxed);
                in_bytes.fetch_add(n as u64, Ordering::Relaxed);
                let _ = shape_tx.send((next_free, buf[..n].to_vec(), inside));
            }
        });
        {
            let mut t = f.tasks.lock().unwrap();
            t.push(shaper);
            t.push(main);
        }
        Ok(f)
    }
}

/// Reads the selected candidate pair through the SCTP transport (the only transport accessor
/// on PeerConnection; a media-only connection has none).
pub async fn selected_pair(pc: &Arc<dyn PeerConnection>) -> TestResult<Option<String>> {
    let Some(sctp) = pc.sctp().await else {
        return Ok(None);
    };
    let pair = sctp
        .transport()
        .ice_transport()
        .get_selected_candidate_pair()
        .await?;
    Ok(pair.map(|p| {
        format!(
            "local {}:{} ({}) <-> remote {}:{} ({})",
            p.local().address,
            p.local().port,
            p.local().typ,
            p.remote().address,
            p.remote().port,
            p.remote().typ
        )
    }))
}
```

`spikes/webrtc-rs/tests/external_candidate.rs`:

```rust
// Spike only (throwaway).
//! Q1. Can the host advertise a UPnP-mapped (external) address so the viewer connects through it?
//!
//! Simulation: the answerer ("host") binds a fixed 127.0.0.1:<port>. A NAT-like forwarder owns
//! 127.0.0.2:40000 (the "router's external port", Linux routes all of 127/8 to lo without setup)
//! and relays to the answerer. The answerer's SDP loses its host candidate and gains
//! `typ srflx 127.0.0.2 40000 raddr 127.0.0.1 rport <port>` instead.
//!
//! Why SDP editing: the async `webrtc` 0.21.0 `PeerConnection` trait has no
//! `add_local_candidate` (only the sans-I/O `rtc` core does, and the driver owns that core), and
//! `SettingEngineBuilder::with_nat_1to1_ips` is stored but never read anywhere in `rtc` or
//! `webrtc` 0.21.0 (and could only swap the IP, not the port, anyway).
//!
//! The offerer's host candidates are stripped as well, so the answerer cannot check the
//! offerer directly: the only possible path is offerer → forwarder → answerer. The answerer
//! learns the forwarder's inside address as a peer-reflexive candidate from incoming checks.

mod common;

use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use common::*;

const PUBLIC: &str = "127.0.0.2:40000";

#[tokio::test(flavor = "multi_thread")]
async fn srflx_candidate_injected_via_sdp_connects_through_forwarder() -> TestResult {
    let port = free_udp_port("127.0.0.1");
    let answerer_addr: SocketAddr = format!("127.0.0.1:{port}").parse()?;
    let fwd = Forwarder::start(PUBLIC, answerer_addr).await?;

    let (offerer, mut oe) =
        build_peer("offerer", "127.0.0.1:0", None, setting_engine().build()).await?;
    let (answerer, mut ae) = build_peer(
        "answerer",
        &answerer_addr.to_string(),
        None,
        setting_engine().build(),
    )
    .await?;

    let dc = offerer.create_data_channel("q1", None).await?;
    let mut off_reader = spawn_dc_reader(dc.clone());

    let t0 = Instant::now();
    let offer = make_offer(&offerer, &mut oe, None).await?;
    let offer = with_sdp(&offer, strip_candidates(&offer.sdp))?;

    let answer = make_answer(&answerer, &mut ae, offer).await?;
    let hosts = host_candidates(&answer.sdp);
    println!("answerer host candidates: {hosts:?}");
    assert_eq!(hosts, vec![answerer_addr], "answerer bound the fixed port");
    let srflx = srflx_candidate_line(fwd.public_addr, answerer_addr);
    println!("injected: {srflx}");
    let answer = with_sdp(&answer, replace_candidates(&answer.sdp, &[srflx]))?;
    println!("answer as sent:\n{}", answer.sdp);
    offerer.set_remote_description(answer).await?;

    wait_connected(&mut oe, CONNECT_TIMEOUT).await?;
    wait_connected(&mut ae, CONNECT_TIMEOUT).await?;
    println!("connected in {:?}", t0.elapsed());

    off_reader.wait_open(Duration::from_secs(10)).await?;
    let adc = tokio::time::timeout(Duration::from_secs(10), ae.data_channels.recv())
        .await?
        .ok_or("no data channel on answerer")?;
    let mut ans_reader = spawn_dc_reader(adc.clone());

    dc.send_text("via-upnp").await?;
    let got = ans_reader.recv(Duration::from_secs(10)).await?;
    assert_eq!(got, b"via-upnp");
    adc.send_text("reply").await?;
    let got = off_reader.recv(Duration::from_secs(10)).await?;
    assert_eq!(got, b"reply");

    let off_pair = selected_pair(&offerer).await?;
    let ans_pair = selected_pair(&answerer).await?;
    println!("offerer selected pair: {off_pair:?}");
    println!("answerer selected pair: {ans_pair:?}");
    let inbound = fwd.inbound_forwarded.load(Ordering::Relaxed);
    let outbound = fwd.outbound_forwarded.load(Ordering::Relaxed);
    println!("forwarder datagrams: inbound {inbound}, outbound {outbound}");

    assert!(
        inbound > 0 && outbound > 0,
        "traffic went through the forwarder"
    );
    let off_pair = off_pair.ok_or("offerer has no selected pair")?;
    assert!(
        off_pair.contains("remote 127.0.0.2:40000"),
        "offerer's selected remote is the injected srflx address: {off_pair}"
    );

    offerer.close().await?;
    answerer.close().await?;
    Ok(())
}
```

`spikes/webrtc-rs/tests/ice_restart.rs`:

```rust
// Spike only (throwaway).
//! Q2. Does ICE restart recover the connection in place, keeping the same DataChannel, and how
//! long does it take on loopback?
//!
//! Three variants:
//! - `restart_ice()` then `create_offer(None)` (the W3C restartIce path),
//! - `create_offer(Some(RTCOfferOptions { ice_restart: true }))`,
//! - `restart_ice()` with `with_discard_local_candidates_during_ice_restart(true)`, which also
//!   replaces the UDP sockets (the "network changed" case).
//!
//! Each asserts: new ICE ufrag, state back to Connected, and the data channel created before
//! the restart delivers messages both ways after it.

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::*;
use rtc::peer_connection::configuration::RTCOfferOptions;
use webrtc::peer_connection::{PeerConnection, RTCPeerConnectionState};

#[derive(Clone, Copy, Debug)]
enum How {
    RestartIceApi,
    OfferOption,
}

async fn local_ufrag(pc: &Arc<dyn PeerConnection>) -> TestResult<String> {
    let sctp = pc.sctp().await.ok_or("no SCTP transport")?;
    let params = sctp
        .transport()
        .ice_transport()
        .get_local_parameters()
        .await?
        .ok_or("no local ICE parameters")?;
    Ok(params.username_fragment)
}

async fn run(how: How, discard_local_candidates: bool) -> TestResult {
    let se = || {
        setting_engine()
            .with_discard_local_candidates_during_ice_restart(discard_local_candidates)
            .build()
    };
    let (offerer, mut oe) = build_peer("offerer", "127.0.0.1:0", None, se()).await?;
    let (answerer, mut ae) = build_peer("answerer", "127.0.0.1:0", None, se()).await?;

    let dc = offerer.create_data_channel("q2", None).await?;
    let mut off_reader = spawn_dc_reader(dc.clone());

    let t0 = Instant::now();
    negotiate(&offerer, &mut oe, &answerer, &mut ae, None).await?;
    wait_connected(&mut oe, CONNECT_TIMEOUT).await?;
    wait_connected(&mut ae, CONNECT_TIMEOUT).await?;
    off_reader.wait_open(Duration::from_secs(10)).await?;
    let adc = tokio::time::timeout(Duration::from_secs(10), ae.data_channels.recv())
        .await?
        .ok_or("no data channel on answerer")?;
    let mut ans_reader = spawn_dc_reader(adc.clone());
    println!("initial connect + open: {:?}", t0.elapsed());

    dc.send_text("before").await?;
    assert_eq!(ans_reader.recv(Duration::from_secs(5)).await?, b"before");
    adc.send_text("before-reply").await?;
    assert_eq!(
        off_reader.recv(Duration::from_secs(5)).await?,
        b"before-reply"
    );

    let ufrag_before = local_ufrag(&offerer).await?;
    println!("pair before: {:?}", selected_pair(&offerer).await?);

    // ── restart ──
    let t_restart = Instant::now();
    let options = match how {
        How::RestartIceApi => {
            offerer.restart_ice().await?;
            None
        }
        How::OfferOption => Some(RTCOfferOptions { ice_restart: true }),
    };
    negotiate(&offerer, &mut oe, &answerer, &mut ae, options).await?;
    let t_signaled = t_restart.elapsed();

    // Wait for Connected again (it may never have left Connected on loopback; then this
    // returns at once and the message round trip below is the real proof).
    wait_connected(&mut oe, CONNECT_TIMEOUT).await?;
    wait_connected(&mut ae, CONNECT_TIMEOUT).await?;

    dc.send_text("after").await?;
    assert_eq!(ans_reader.recv(Duration::from_secs(15)).await?, b"after");
    let t_first_msg = t_restart.elapsed();
    adc.send_text("after-reply").await?;
    assert_eq!(
        off_reader.recv(Duration::from_secs(15)).await?,
        b"after-reply"
    );
    let t_round_trip = t_restart.elapsed();

    let ufrag_after = local_ufrag(&offerer).await?;
    println!("pair after: {:?}", selected_pair(&offerer).await?);
    println!(
        "[{how:?}, discard={discard_local_candidates}] ufrag {ufrag_before} -> {ufrag_after}; \
         signaling done {t_signaled:?}, first msg {t_first_msg:?}, round trip {t_round_trip:?}"
    );
    print_history(&oe, t0);
    print_history(&ae, t0);

    assert_ne!(ufrag_before, ufrag_after, "the offer really restarted ICE");
    assert_eq!(*oe.state.borrow(), RTCPeerConnectionState::Connected);
    assert_eq!(*ae.state.borrow(), RTCPeerConnectionState::Connected);

    offerer.close().await?;
    answerer.close().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn restart_via_restart_ice_api() -> TestResult {
    run(How::RestartIceApi, false).await
}

#[tokio::test(flavor = "multi_thread")]
async fn restart_via_offer_options() -> TestResult {
    run(How::OfferOption, false).await
}

#[tokio::test(flavor = "multi_thread")]
async fn restart_with_socket_rebind() -> TestResult {
    run(How::RestartIceApi, true).await
}
```

`spikes/webrtc-rs/tests/h264.rs`:

```rust
// Spike only (throwaway).
//! Q3. H.264 over TrackLocalStaticSample: do large NAL units survive FU-A fragmentation and
//! reassembly byte for byte, and does a PLI from the viewer reach the sending application?
//!
//! Sender: synthetic Annex-B access units (SPS + PPS + IDR, then P slices). IDR and P slices are
//! 5000 bytes, so at the 1200-byte packetizer MTU each becomes ~5 FU-A packets.
//! SPS/PPS stay small on purpose: `H264Payloader` does not send SPS/PPS on their own; it holds
//! them and emits one STAP-A (SPS+PPS) in front of the next NAL, and only `if stap_a.len() <= mtu`
//! (rtc-rtp h264/mod.rs:104). An oversized SPS/PPS would be dropped silently; the unit test at
//! the bottom pins that behaviour.
//!
//! Receiver: `on_track` → `TrackRemote::poll` → `OnRtpPacket` → `H264Packet::depacketize`
//! (Annex-B output, 4-byte start codes) → split into NAL units → compare with what was sent.
//!
//! PLI: the answerer calls `TrackRemote::write_rtcp(PictureLossIndication)`. RTCP reaching the
//! offerer is consumed by the interceptor chain unless something marks it
//! `Attribute::DeliverToApplication`; a custom interceptor (copied in spirit from
//! examples/rtcp-processing) does that for PLI/FIR, and the offerer reads it from
//! `TrackLocal::poll` as `TrackLocalEvent::OnRtcpPacket`.

mod common;

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use bytes::Bytes;
use common::*;
use rtc::interceptor::{Attribute, Interceptor, Packet, Slot, StreamInfo, TaggedPacket};
use rtc::media::Sample;
use rtc::media_stream::MediaStreamTrack;
use rtc::peer_connection::configuration::interceptor_registry::register_default_interceptors;
use rtc::peer_connection::configuration::media_engine::MIME_TYPE_H264;
use rtc::rtcp::payload_feedbacks::full_intra_request::FullIntraRequest;
use rtc::rtcp::payload_feedbacks::picture_loss_indication::PictureLossIndication;
use rtc::rtp::codec::h264::{H264Packet, H264Payloader};
use rtc::rtp::packetizer::{Depacketizer, Payloader};
use rtc::rtp_transceiver::rtp_sender::{
    RTCRtpCodec, RTCRtpCodecParameters, RTCRtpCodingParameters, RTCRtpEncodingParameters,
    RtpCodecKind,
};
use rtc::sansio::Protocol;
use rtc::shared::error::Error;
use tokio::sync::mpsc;
use webrtc::media_stream::Track;
use webrtc::media_stream::track_local::static_sample::TrackLocalStaticSample;
use webrtc::media_stream::track_local::{TrackLocal, TrackLocalEvent};
use webrtc::media_stream::track_remote::TrackRemoteEvent;
use webrtc::peer_connection::{MediaEngine, Registry};

const FRAME: Duration = Duration::from_millis(33);
const BIG: usize = 5000;
const P_FRAMES: usize = 5;

// ───────────── interceptor: surface PLI/FIR to the application ─────────────

#[derive(Default)]
struct KeyframeRequestToApp {
    read_queue: VecDeque<TaggedPacket>,
    write_queue: VecDeque<TaggedPacket>,
}

fn is_keyframe_request(p: &Box<dyn rtc::rtcp::Packet>) -> bool {
    let any = p.as_any();
    any.is::<PictureLossIndication>() || any.is::<FullIntraRequest>()
}

impl Protocol<TaggedPacket, TaggedPacket, ()> for KeyframeRequestToApp {
    type Rout = TaggedPacket;
    type Wout = TaggedPacket;
    type Eout = ();
    type Error = Error;
    type Time = Instant;

    fn handle_read(&mut self, mut msg: TaggedPacket) -> Result<(), Self::Error> {
        if let Packet::Rtcp(packets) = &msg.message.packet {
            let requests: Vec<Box<dyn rtc::rtcp::Packet>> = packets
                .iter()
                .filter(|p| is_keyframe_request(p))
                .cloned()
                .collect();
            if !requests.is_empty() {
                msg.message.packet = Packet::Rtcp(requests);
                msg.message.add(Attribute::DeliverToApplication);
            }
        }
        self.read_queue.push_back(msg);
        Ok(())
    }
    fn poll_read(&mut self) -> Option<Self::Rout> {
        self.read_queue.pop_front()
    }
    fn handle_write(&mut self, msg: TaggedPacket) -> Result<(), Self::Error> {
        self.write_queue.push_back(msg);
        Ok(())
    }
    fn poll_write(&mut self) -> Option<Self::Wout> {
        self.write_queue.pop_front()
    }
}

impl Interceptor for KeyframeRequestToApp {
    fn bind_local_stream(&mut self, _info: &StreamInfo) {}
    fn unbind_local_stream(&mut self, _info: &StreamInfo) {}
    fn bind_remote_stream(&mut self, _info: &StreamInfo) {}
    fn unbind_remote_stream(&mut self, _info: &StreamInfo) {}
}

// ───────────── H.264 helpers ─────────────

fn h264_codec() -> RTCRtpCodec {
    RTCRtpCodec {
        mime_type: MIME_TYPE_H264.to_owned(),
        clock_rate: 90000,
        channels: 0,
        sdp_fmtp_line: "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f"
            .to_owned(),
        rtcp_feedback: vec![],
    }
}

fn media(with_pli_to_app: bool) -> TestResult<(MediaEngine, Registry)> {
    let mut me = MediaEngine::default();
    me.register_codec(
        RTCRtpCodecParameters {
            rtp_codec: h264_codec(),
            payload_type: 102,
            ..Default::default()
        },
        RtpCodecKind::Video,
    )?;
    let mut registry = register_default_interceptors(Registry::new(), &mut me)?;
    if with_pli_to_app {
        // Past JitterBuffer (13_000): read walks the chain forwards, so this is the last
        // interceptor an incoming packet meets before the application.
        registry = registry.with(Slot::from(14_000), KeyframeRequestToApp::default());
    }
    Ok((me, registry))
}

/// NAL unit = header byte + body. Body bytes are 1..=250 so no start code can appear inside.
fn nal(header: u8, len: usize, seed: u8) -> Vec<u8> {
    let mut v = Vec::with_capacity(len);
    v.push(header);
    for i in 1..len {
        v.push(((i + seed as usize) % 250 + 1) as u8);
    }
    v
}

fn annex_b(nals: &[&[u8]]) -> Bytes {
    let mut out = Vec::new();
    for n in nals {
        out.extend_from_slice(&[0, 0, 0, 1]);
        out.extend_from_slice(n);
    }
    Bytes::from(out)
}

/// Splits an Annex-B stream on 3- or 4-byte start codes.
fn split_annex_b(data: &[u8]) -> Vec<Vec<u8>> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 <= data.len() {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            starts.push(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }
    let mut out = Vec::new();
    for (k, &s) in starts.iter().enumerate() {
        let mut e = if k + 1 < starts.len() {
            starts[k + 1] - 3
        } else {
            data.len()
        };
        while e > s && data[e - 1] == 0 {
            e -= 1; // leading zero of a 4-byte start code belongs to no NAL
        }
        out.push(data[s..e].to_vec());
    }
    out
}

// ───────────── test ─────────────

#[tokio::test(flavor = "multi_thread")]
async fn h264_fua_roundtrip_and_pli_to_sender_app() -> TestResult {
    let (offerer, mut oe) = build_peer(
        "offerer",
        "127.0.0.1:0",
        Some(media(true)?),
        setting_engine().build(),
    )
    .await?;
    let (answerer, mut ae) = build_peer(
        "answerer",
        "127.0.0.1:0",
        Some(media(false)?),
        setting_engine().build(),
    )
    .await?;

    let ssrc: u32 = 0x1234_5678;
    let track = Arc::new(TrackLocalStaticSample::new(
        Instant::now(),
        MediaStreamTrack::new(
            "q3-stream".to_owned(),
            "q3-video".to_owned(),
            "q3-label".to_owned(),
            RtpCodecKind::Video,
            vec![RTCRtpEncodingParameters {
                rtp_coding_parameters: RTCRtpCodingParameters {
                    ssrc: Some(ssrc),
                    ..Default::default()
                },
                codec: h264_codec(),
                ..Default::default()
            }],
        ),
    )?);
    let sender = offerer
        .add_track(Arc::clone(&track) as Arc<dyn TrackLocal>)
        .await?;

    negotiate(&offerer, &mut oe, &answerer, &mut ae, None).await?;
    wait_connected(&mut oe, CONNECT_TIMEOUT).await?;
    wait_connected(&mut ae, CONNECT_TIMEOUT).await?;

    let pt = sender
        .get_parameters()
        .await?
        .rtp_parameters
        .codecs
        .first()
        .map(|c| c.payload_type)
        .ok_or("sender has no negotiated codec")?;
    assert_eq!(track.ssrcs().await, vec![ssrc]);
    println!("negotiated payload type {pt}");

    // Offerer: count keyframe requests surfaced through TrackLocal::poll.
    let pli_seen = Arc::new(AtomicU32::new(0));
    {
        let pli_seen = Arc::clone(&pli_seen);
        let t = Arc::clone(&track);
        tokio::spawn(async move {
            while let Some(ev) = t.poll().await {
                if let TrackLocalEvent::OnRtcpPacket(pkts) = ev {
                    let n = pkts
                        .iter()
                        .filter(|p| p.as_any().is::<PictureLossIndication>())
                        .count();
                    pli_seen.fetch_add(n as u32, Ordering::SeqCst);
                }
            }
        });
    }

    // Warm-up: the TrackRemote is created on the first RTP packet, and events and media travel
    // in separate queues, so the earliest packets are not guaranteed to reach poll(). Send small
    // single-NAL P slices until the answerer has its track, then send the real sequence.
    let warm = nal(0x41, 20, 7);
    let (nal_tx, mut nal_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let remote = loop {
        track
            .write_sample(
                ssrc,
                pt,
                &Sample {
                    data: annex_b(&[&warm]),
                    duration: FRAME,
                    ..Sample::new(Instant::now())
                },
                &[],
            )
            .await?;
        if let Ok(Some(t)) = tokio::time::timeout(FRAME, ae.tracks.recv()).await {
            break t;
        }
    };
    println!("answerer on_track, ssrcs {:?}", remote.ssrcs().await);

    // Answerer: depacketize everything into NAL units.
    {
        let remote = Arc::clone(&remote);
        tokio::spawn(async move {
            let mut depack = H264Packet::default();
            while let Some(ev) = remote.poll().await {
                match ev {
                    TrackRemoteEvent::OnRtpPacket(pkt) => match depack.depacketize(&pkt.payload) {
                        Ok(out) if !out.is_empty() => {
                            for n in split_annex_b(&out) {
                                let _ = nal_tx.send(n);
                            }
                        }
                        Ok(_) => {} // middle of an FU-A
                        Err(e) => eprintln!("depacketize error: {e}"),
                    },
                    TrackRemoteEvent::OnEnded => break,
                    _ => {}
                }
            }
        });
    }
    // Answerer: PLI every 200 ms.
    {
        let remote = Arc::clone(&remote);
        let media_ssrc = remote.ssrcs().await.first().copied().unwrap_or(ssrc);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_millis(200));
            loop {
                tick.tick().await;
                let pli = PictureLossIndication {
                    sender_ssrc: 0,
                    media_ssrc,
                };
                if remote.write_rtcp(vec![Box::new(pli)]).await.is_err() {
                    break;
                }
            }
        });
    }

    // The real sequence: AU0 = SPS+PPS+IDR, then P slices, one AU per frame.
    let sps = nal(0x67, 12, 1);
    let pps = nal(0x68, 5, 2);
    let idr = nal(0x65, BIG, 3);
    let ps: Vec<Vec<u8>> = (0..P_FRAMES)
        .map(|k| nal(0x41, BIG, 10 + k as u8))
        .collect();

    let mut expected: Vec<Vec<u8>> = vec![sps.clone(), pps.clone(), idr.clone()];
    expected.extend(ps.iter().cloned());

    let mut aus: Vec<Bytes> = vec![annex_b(&[&sps, &pps, &idr])];
    aus.extend(ps.iter().map(|p| annex_b(&[p])));
    for au in aus {
        track
            .write_sample(
                ssrc,
                pt,
                &Sample {
                    data: au,
                    duration: FRAME,
                    ..Sample::new(Instant::now())
                },
                &[],
            )
            .await?;
        tokio::time::sleep(FRAME).await;
    }

    // Collect: skip warm-up NALs until the SPS, then take expected.len() NALs.
    let mut got: Vec<Vec<u8>> = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while got.len() < expected.len() {
        let n = tokio::time::timeout_at(deadline, nal_rx.recv())
            .await
            .map_err(|_| format!("only {} of {} NALs arrived", got.len(), expected.len()))?
            .ok_or("receiver ended")?;
        if got.is_empty() && n.first() != Some(&0x67) {
            continue;
        }
        got.push(n);
    }
    for (i, (g, e)) in got.iter().zip(expected.iter()).enumerate() {
        println!(
            "NAL {i}: type {} len {} (sent {})",
            g[0] & 0x1f,
            g.len(),
            e.len()
        );
    }
    assert_eq!(got, expected, "reassembled NAL units equal what was sent");

    // PLI reached the sender's application.
    let deadline = Instant::now() + Duration::from_secs(5);
    while pli_seen.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let plis = pli_seen.load(Ordering::SeqCst);
    println!("PLIs seen by offerer app: {plis}");
    assert!(plis >= 1, "PLI surfaced as TrackLocalEvent::OnRtcpPacket");

    offerer.close().await?;
    answerer.close().await?;
    Ok(())
}

/// Pins the payloader behaviour described at the top: SPS/PPS are only ever sent as a STAP-A in
/// front of the next NAL, and that STAP-A is dropped if it exceeds the MTU.
#[test]
fn payloader_drops_oversized_sps_pps_stap_a() -> TestResult {
    let mtu = 1200;
    let small = annex_b(&[&nal(0x67, 12, 1), &nal(0x68, 5, 2), &nal(0x65, 100, 3)]);
    let out = H264Payloader::default().payload(mtu, &small)?;
    assert_eq!(out.len(), 2, "STAP-A(SPS,PPS) + IDR");
    assert_eq!(out[0][0] & 0x1f, 24, "first payload is STAP-A");

    let big = annex_b(&[&nal(0x67, BIG, 1), &nal(0x68, 5, 2), &nal(0x65, 100, 3)]);
    let out = H264Payloader::default().payload(mtu, &big)?;
    assert_eq!(
        out.len(),
        1,
        "oversized SPS+PPS STAP-A silently dropped, only IDR left"
    );
    assert_eq!(out[0][0] & 0x1f, 5);
    Ok(())
}
```

`spikes/webrtc-rs/tests/data_channel.rs`:

```rust
// Spike only (throwaway).
//! Q4. Under 20 % packet loss, does a reliable ordered channel deliver everything in order,
//! and does an unordered `max_retransmits: Some(0)` channel deliver some but not all?
//!
//! Loss: a forwarder in front of the answerer (same SDP rewrite as external_candidate.rs:
//! offerer candidates stripped, answerer's host candidate replaced by the forwarder address),
//! dropping every 5th datagram offerer → answerer once both channels are open.
//! Messages are 1000 bytes so SCTP cannot bundle several into one datagram; each loss then
//! costs exactly one message on the unreliable channel.

mod common;

use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use bytes::BytesMut;
use common::*;
use webrtc::data_channel::RTCDataChannelInit;

const N: u32 = 200;
const MSG_LEN: usize = 1000;

fn message(i: u32) -> BytesMut {
    let mut b = BytesMut::zeroed(MSG_LEN);
    b[..4].copy_from_slice(&i.to_be_bytes());
    b
}

fn index(m: &[u8]) -> u32 {
    u32::from_be_bytes(m[..4].try_into().unwrap())
}

#[tokio::test(flavor = "multi_thread")]
async fn reliable_vs_unreliable_under_loss() -> TestResult {
    let (offerer, mut oe) =
        build_peer("offerer", "127.0.0.1:0", None, setting_engine().build()).await?;
    let (answerer, mut ae) =
        build_peer("answerer", "127.0.0.1:0", None, setting_engine().build()).await?;

    let reliable = offerer.create_data_channel("reliable", None).await?;
    let unreliable = offerer
        .create_data_channel(
            "unreliable",
            Some(RTCDataChannelInit {
                ordered: false,
                max_retransmits: Some(0),
                ..Default::default()
            }),
        )
        .await?;
    let mut rel_off = spawn_dc_reader(reliable.clone());
    let mut unrel_off = spawn_dc_reader(unreliable.clone());

    // Signaling with the path forced through the forwarder.
    let offer = make_offer(&offerer, &mut oe, None).await?;
    let offer = with_sdp(&offer, strip_candidates(&offer.sdp))?;
    let answer = make_answer(&answerer, &mut ae, offer).await?;
    let target = *host_candidates(&answer.sdp)
        .first()
        .ok_or("answerer has no host candidate")?;
    let fwd = Forwarder::start("127.0.0.2:0", target).await?;
    let srflx = srflx_candidate_line(fwd.public_addr, target);
    let answer = with_sdp(&answer, replace_candidates(&answer.sdp, &[srflx]))?;
    offerer.set_remote_description(answer).await?;

    wait_connected(&mut oe, CONNECT_TIMEOUT).await?;
    wait_connected(&mut ae, CONNECT_TIMEOUT).await?;
    rel_off.wait_open(Duration::from_secs(10)).await?;
    unrel_off.wait_open(Duration::from_secs(10)).await?;

    // Answerer side: route by label.
    let mut rel_ans = None;
    let mut unrel_ans = None;
    while rel_ans.is_none() || unrel_ans.is_none() {
        let dc = tokio::time::timeout(Duration::from_secs(10), ae.data_channels.recv())
            .await?
            .ok_or("answerer data channel stream ended")?;
        let label = dc.label().await?;
        println!(
            "answerer got '{label}': ordered={} max_retransmits={:?}",
            dc.ordered().await?,
            dc.max_retransmits().await?
        );
        match label.as_str() {
            "reliable" => rel_ans = Some(spawn_dc_reader(dc)),
            "unreliable" => unrel_ans = Some(spawn_dc_reader(dc)),
            other => return Err(format!("unexpected channel {other}").into()),
        }
    }
    let (mut rel_ans, mut unrel_ans) = (rel_ans.unwrap(), unrel_ans.unwrap());

    fwd.drop_every_nth.store(5, Ordering::Relaxed);

    // ── reliable ordered ──
    let t = Instant::now();
    for i in 0..N {
        reliable.send(message(i)).await?;
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    let mut got = Vec::new();
    while got.len() < N as usize {
        let m = rel_ans.recv(Duration::from_secs(30)).await?;
        assert_eq!(m.len(), MSG_LEN);
        got.push(index(&m));
    }
    let rel_time = t.elapsed();
    assert_eq!(got, (0..N).collect::<Vec<_>>(), "reliable: all, in order");

    // ── unreliable unordered, max_retransmits 0 ──
    let dropped_before = fwd.inbound_dropped.load(Ordering::Relaxed);
    for i in 0..N {
        unreliable.send(message(i)).await?;
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    let mut seen = Vec::new();
    // Nothing is retransmitted, so after 3 quiet seconds nothing more will come.
    while let Ok(m) = unrel_ans.recv(Duration::from_secs(3)).await {
        seen.push(index(&m));
    }
    let dropped_during = fwd.inbound_dropped.load(Ordering::Relaxed) - dropped_before;
    let unique: HashSet<u32> = seen.iter().copied().collect();
    let out_of_order = seen.windows(2).filter(|w| w[1] < w[0]).count();

    println!(
        "reliable: {N}/{N} in order in {rel_time:?}; unreliable: {} received ({} unique, \
         {out_of_order} out of order), {dropped_during} datagrams dropped meanwhile; \
         forwarder totals: fwd {} drop {}",
        seen.len(),
        unique.len(),
        fwd.inbound_forwarded.load(Ordering::Relaxed),
        fwd.inbound_dropped.load(Ordering::Relaxed),
    );
    assert_eq!(
        unique.len(),
        seen.len(),
        "no duplicates on the unreliable channel"
    );
    assert!(
        !seen.is_empty() && seen.len() < N as usize,
        "unreliable: some but not all delivered (got {})",
        seen.len()
    );

    offerer.close().await?;
    answerer.close().await?;
    Ok(())
}
```

`spikes/webrtc-rs/tests/bwe.rs`:

```rust
// Spike only (throwaway).
//! Q5. Does GCC (send-side, TWCC feedback) track a bottleneck: settle near 1 Mbps, then climb
//! when the link opens to 5 Mbps?
//!
//! - Offerer (sender): `configure_congestion_control(Registry::new(), estimator, Twcc, &mut me)`
//!   then `register_default_interceptors`, like examples/bandwidth-estimation-from-disk.
//!   The estimator is `Gcc::new(initial, min, max)` wrapped so every update publishes
//!   `target_bitrate()` to an `AtomicU64` the test can read.
//! - Answerer (receiver): `register_default_interceptors` only; it includes the TWCC receiver
//!   that sends the feedback GCC needs.
//! - Path: forwarder in front of the answerer (SDP rewrite as in external_candidate.rs) with a
//!   rate limit and a 200 ms drop-tail queue on the media direction. Phase 1: 1 Mbps; phase 2:
//!   5 Mbps.
//! - Source: synthetic H.264 at 30 fps whose frame size follows the current target, like an
//!   encoder under rate control (GCC only grows while the sender actually fills the target).
//!
//! Prints a per-second timeline: target, bytes delivered past the bottleneck, drops.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use bytes::Bytes;
use common::*;
use rtc::interceptor::{BandwidthEstimator, EstimatorStats, Gcc, PacketReport};
use rtc::media::Sample;
use rtc::media_stream::MediaStreamTrack;
use rtc::peer_connection::configuration::interceptor_registry::{
    CongestionFeedback, configure_congestion_control, register_default_interceptors,
};
use rtc::peer_connection::configuration::media_engine::MIME_TYPE_H264;
use rtc::rtp_transceiver::rtp_sender::{
    RTCRtpCodec, RTCRtpCodecParameters, RTCRtpCodingParameters, RTCRtpEncodingParameters,
    RtpCodecKind,
};
use webrtc::media_stream::track_local::TrackLocal;
use webrtc::media_stream::track_local::static_sample::TrackLocalStaticSample;
use webrtc::media_stream::track_remote::TrackRemoteEvent;
use webrtc::peer_connection::{MediaEngine, Registry};

const INITIAL_BPS: f64 = 2_500_000.0; // above phase-1 capacity, so the decrease path is exercised
const MIN_BPS: f64 = 100_000.0;
const MAX_BPS: f64 = 10_000_000.0;
const PHASE1: Duration = Duration::from_secs(20);
const PHASE2: Duration = Duration::from_secs(20);
const FPS: u32 = 30;

// ───────────── estimator wrapper (as in the example) ─────────────

struct ReportingEstimator<E: BandwidthEstimator> {
    inner: E,
    target: Arc<AtomicU64>,
}

impl<E: BandwidthEstimator> ReportingEstimator<E> {
    fn new(inner: E) -> (Self, Arc<AtomicU64>) {
        let target = Arc::new(AtomicU64::new(inner.target_bitrate().to_bits()));
        (
            Self {
                inner,
                target: Arc::clone(&target),
            },
            target,
        )
    }
    fn publish(&self) {
        self.target
            .store(self.inner.target_bitrate().to_bits(), Ordering::Relaxed);
    }
}

impl<E: BandwidthEstimator> BandwidthEstimator for ReportingEstimator<E> {
    fn on_reports(&mut self, now: Instant, reports: &[PacketReport]) {
        self.inner.on_reports(now, reports);
        self.publish();
    }
    fn target_bitrate(&self) -> f64 {
        self.inner.target_bitrate()
    }
    fn handle_timeout(&mut self, now: Instant) {
        self.inner.handle_timeout(now);
        self.publish();
    }
    fn poll_timeout(&self) -> Option<Instant> {
        self.inner.poll_timeout()
    }
    fn stats(&self) -> EstimatorStats {
        self.inner.stats()
    }
}

fn h264_codec() -> RTCRtpCodec {
    RTCRtpCodec {
        mime_type: MIME_TYPE_H264.to_owned(),
        clock_rate: 90000,
        channels: 0,
        sdp_fmtp_line: "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f"
            .to_owned(),
        rtcp_feedback: vec![],
    }
}

fn media_engine() -> TestResult<MediaEngine> {
    let mut me = MediaEngine::default();
    me.register_codec(
        RTCRtpCodecParameters {
            rtp_codec: h264_codec(),
            payload_type: 102,
            ..Default::default()
        },
        RtpCodecKind::Video,
    )?;
    Ok(me)
}

fn frame(bytes: usize, seq: u32) -> Bytes {
    // One non-IDR slice NAL; body bytes non-zero so no start code appears inside.
    let len = bytes.max(16);
    let mut v = Vec::with_capacity(len + 4);
    v.extend_from_slice(&[0, 0, 0, 1, 0x41]);
    for i in 0..len {
        v.push(((i as u32 + seq) % 250 + 1) as u8);
    }
    Bytes::from(v)
}

fn load(a: &AtomicU64) -> f64 {
    f64::from_bits(a.load(Ordering::Relaxed))
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

#[tokio::test(flavor = "multi_thread")]
async fn gcc_follows_bottleneck() -> TestResult {
    // Sender with congestion control.
    let mut me = media_engine()?;
    let (estimator, target) = ReportingEstimator::new(Gcc::new(INITIAL_BPS, MIN_BPS, MAX_BPS));
    let registry = configure_congestion_control(
        Registry::new(),
        estimator,
        CongestionFeedback::Twcc,
        &mut me,
    )?;
    let registry = register_default_interceptors(registry, &mut me)?;
    let (offerer, mut oe) = build_peer(
        "offerer",
        "127.0.0.1:0",
        Some((me, registry)),
        setting_engine().build(),
    )
    .await?;

    // Receiver.
    let mut me = media_engine()?;
    let registry = register_default_interceptors(Registry::new(), &mut me)?;
    let (answerer, mut ae) = build_peer(
        "answerer",
        "127.0.0.1:0",
        Some((me, registry)),
        setting_engine().build(),
    )
    .await?;

    let ssrc: u32 = 0x0BAD_CAFE;
    let track = Arc::new(TrackLocalStaticSample::new(
        Instant::now(),
        MediaStreamTrack::new(
            "q5-stream".to_owned(),
            "q5-video".to_owned(),
            "q5-label".to_owned(),
            RtpCodecKind::Video,
            vec![RTCRtpEncodingParameters {
                rtp_coding_parameters: RTCRtpCodingParameters {
                    ssrc: Some(ssrc),
                    ..Default::default()
                },
                codec: h264_codec(),
                ..Default::default()
            }],
        ),
    )?);
    let sender = offerer
        .add_track(Arc::clone(&track) as Arc<dyn TrackLocal>)
        .await?;

    // Signaling through the forwarder.
    let offer = make_offer(&offerer, &mut oe, None).await?;
    let offer = with_sdp(&offer, strip_candidates(&offer.sdp))?;
    let answer = make_answer(&answerer, &mut ae, offer).await?;
    let target_addr = *host_candidates(&answer.sdp)
        .first()
        .ok_or("answerer has no host candidate")?;
    let fwd = Forwarder::start("127.0.0.2:0", target_addr).await?;
    fwd.rate_bps.store(1_000_000, Ordering::Relaxed);
    let srflx = srflx_candidate_line(fwd.public_addr, target_addr);
    let answer = with_sdp(&answer, replace_candidates(&answer.sdp, &[srflx]))?;
    assert!(
        answer.sdp.contains("transport-cc"),
        "answer negotiates TWCC feedback"
    );
    offerer.set_remote_description(answer).await?;
    wait_connected(&mut oe, CONNECT_TIMEOUT).await?;
    wait_connected(&mut ae, CONNECT_TIMEOUT).await?;

    let pt = sender
        .get_parameters()
        .await?
        .rtp_parameters
        .codecs
        .first()
        .map(|c| c.payload_type)
        .ok_or("sender has no negotiated codec")?;

    // Receiver: drain RTP so the track queue never backs up; count payload bytes.
    let rx_bytes = Arc::new(AtomicU64::new(0));
    {
        let rx_bytes = Arc::clone(&rx_bytes);
        let mut tracks_rx =
            std::mem::replace(&mut ae.tracks, tokio::sync::mpsc::unbounded_channel().1);
        tokio::spawn(async move {
            while let Some(remote) = tracks_rx.recv().await {
                let rx_bytes = Arc::clone(&rx_bytes);
                tokio::spawn(async move {
                    while let Some(ev) = remote.poll().await {
                        match ev {
                            TrackRemoteEvent::OnRtpPacket(p) => {
                                rx_bytes.fetch_add(p.payload.len() as u64, Ordering::Relaxed);
                            }
                            TrackRemoteEvent::OnEnded => break,
                            _ => {}
                        }
                    }
                });
            }
        });
    }

    // Sender: frame size follows the current target.
    let running = Arc::new(AtomicBool::new(true));
    let tx_bytes = Arc::new(AtomicU64::new(0));
    let send_task = {
        let (track, target, running, tx_bytes) = (
            Arc::clone(&track),
            Arc::clone(&target),
            Arc::clone(&running),
            Arc::clone(&tx_bytes),
        );
        tokio::spawn(async move {
            let period = Duration::from_secs(1) / FPS;
            let mut tick = tokio::time::interval(period);
            let mut seq = 0u32;
            while running.load(Ordering::Relaxed) {
                tick.tick().await;
                let bytes = (load(&target) / 8.0 / FPS as f64) as usize;
                let sample = Sample {
                    data: frame(bytes, seq),
                    duration: period,
                    ..Sample::new(Instant::now())
                };
                if track.write_sample(ssrc, pt, &sample, &[]).await.is_err() {
                    break;
                }
                tx_bytes.fetch_add(bytes as u64, Ordering::Relaxed);
                seq = seq.wrapping_add(1);
            }
        })
    };

    // Timeline.
    let t0 = Instant::now();
    let mut phase1 = Vec::new();
    let mut phase2 = Vec::new();
    let (mut last_rx, mut last_tx, mut last_drop) = (0u64, 0u64, 0u64);
    println!("  t   cap(Mbps)  target(Mbps)  sent(Mbps)  delivered(Mbps)  drops");
    let total = (PHASE1 + PHASE2).as_secs();
    for s in 1..=total {
        if s == PHASE1.as_secs() + 1 {
            fwd.rate_bps.store(5_000_000, Ordering::Relaxed);
        }
        tokio::time::sleep_until(tokio::time::Instant::from_std(t0 + Duration::from_secs(s))).await;
        let rx = rx_bytes.load(Ordering::Relaxed);
        let tx = tx_bytes.load(Ordering::Relaxed);
        let dr = fwd.inbound_dropped.load(Ordering::Relaxed);
        let tgt = load(&target);
        let cap = fwd.rate_bps.load(Ordering::Relaxed) as f64;
        println!(
            "{s:>3}   {:>8.2}   {:>10.2}   {:>9.2}   {:>14.2}   {:>5}",
            cap / 1e6,
            tgt / 1e6,
            (tx - last_tx) as f64 * 8.0 / 1e6,
            (rx - last_rx) as f64 * 8.0 / 1e6,
            dr - last_drop
        );
        (last_rx, last_tx, last_drop) = (rx, tx, dr);
        if s <= PHASE1.as_secs() {
            phase1.push(tgt);
        } else {
            phase2.push(tgt);
        }
    }
    running.store(false, Ordering::Relaxed);
    let _ = send_task.await;

    // Loose assertions: settle under ~1.2 Mbps with a 1 Mbps link (median of the last 5 s),
    // then rise clearly once the link is 5 Mbps.
    let p1_tail = median(phase1[phase1.len() - 5..].to_vec());
    let p2_max = phase2.iter().cloned().fold(0.0, f64::max);
    let p2_tail = median(phase2[phase2.len() - 5..].to_vec());
    println!(
        "phase1 tail median {:.2} Mbps; phase2 max {:.2} Mbps, tail median {:.2} Mbps",
        p1_tail / 1e6,
        p2_max / 1e6,
        p2_tail / 1e6
    );
    assert!(p1_tail < 1_200_000.0, "phase 1 settles below ~1.2 Mbps");
    assert!(
        p2_max > p1_tail * 1.5,
        "phase 2 estimate increases once the bottleneck opens"
    );

    offerer.close().await?;
    answerer.close().await?;
    Ok(())
}
```


---

## 부록 F. `spikes/ipv6-pair`

`spikes/ipv6-pair/Cargo.toml`:

```toml
[package]
name = "ipv6-pair"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
str0m = { version = "=0.23.1", default-features = false, features = ["aws-lc-rs"] }
```

`spikes/ipv6-pair/src/main.rs`:

```rust
//! Spike only (throwaway): two str0m peers in one process over REAL UDP sockets.
//! Usage: ipv6_pair <ip>   e.g. ipv6_pair ::1   or   ipv6_pair 2001:db8::1234 (an address from ipconfig)
//! Binds two UDP sockets on <ip>:0, uses them as host candidates, opens a data channel and
//! exchanges ping/pong. Prints "RESULT ip=<ip> ok=<bool> ms=<elapsed>".
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use str0m::change::{SdpAnswer, SdpOffer};
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, Event, Input, Output, Rtc};

struct Peer {
    rtc: Rtc,
    sock: UdpSocket,
    addr: SocketAddr,
    got: Vec<Vec<u8>>,
    open: Option<str0m::channel::ChannelId>,
}

impl Peer {
    fn new(ip: IpAddr) -> Peer {
        let sock = UdpSocket::bind(SocketAddr::new(ip, 0)).expect("bind");
        sock.set_read_timeout(Some(Duration::from_millis(5))).unwrap();
        let addr = sock.local_addr().unwrap();
        let mut rtc = Rtc::builder().build(Instant::now());
        rtc.add_local_candidate(Candidate::host(addr, "udp").expect("candidate"));
        Peer { rtc, sock, addr, got: vec![], open: None }
    }

    /// Drive output until the next timeout; send transmits on the real socket.
    fn drive(&mut self) -> Instant {
        loop {
            match self.rtc.poll_output().expect("poll_output") {
                Output::Timeout(t) => return t,
                Output::Transmit(t) => {
                    self.sock.send_to(&t.contents, t.destination).expect("send_to");
                }
                Output::Event(Event::ChannelOpen(id, _)) => self.open = Some(id),
                Output::Event(Event::ChannelData(d)) => self.got.push(d.data),
                Output::Event(_) => {}
            }
        }
    }

    /// Read at most one datagram (5 ms timeout) and feed it, then feed the clock.
    fn pump(&mut self) {
        let mut buf = vec![0u8; 2000];
        if let Ok((n, source)) = self.sock.recv_from(&mut buf) {
            let input = Input::Receive(
                Instant::now(),
                Receive {
                    proto: Protocol::Udp,
                    source,
                    destination: self.addr,
                    contents: buf[..n].try_into().expect("datagram"),
                },
            );
            self.rtc.handle_input(input).expect("receive");
        }
        self.rtc.handle_input(Input::Timeout(Instant::now())).expect("timeout");
    }
}

fn main() {
    let ip: IpAddr = std::env::args().nth(1).unwrap_or_else(|| "::1".into()).parse().expect("ip");
    let mut l = Peer::new(ip);
    let mut r = Peer::new(ip);
    println!("L={} R={}", l.addr, r.addr);

    let mut api = l.rtc.sdp_api();
    let cid = api.add_channel("ping".into());
    let (offer, pending) = api.apply().expect("offer");
    let offer = SdpOffer::from_sdp_string(&offer.to_sdp_string()).unwrap();
    let answer = r.rtc.sdp_api().accept_offer(offer).expect("accept_offer");
    let answer = SdpAnswer::from_sdp_string(&answer.to_sdp_string()).unwrap();
    l.rtc.sdp_api().accept_answer(pending, answer).expect("accept_answer");

    let start = Instant::now();
    let mut sent = false;
    let mut ok = false;
    while start.elapsed() < Duration::from_secs(10) {
        l.drive();
        r.drive();
        l.pump();
        r.pump();
        if !sent && l.open == Some(cid) {
            l.rtc.channel(cid).expect("channel").write(false, b"ping").expect("write");
            sent = true;
        }
        if let Some(id) = r.open {
            if r.got.iter().any(|d| d == b"ping") && !r.got.iter().any(|d| d == b"pong-sent") {
                r.rtc.channel(id).expect("r channel").write(false, b"pong").expect("pong");
                r.got.push(b"pong-sent".to_vec());
            }
        }
        if l.got.iter().any(|d| d == b"pong") {
            ok = true;
            break;
        }
    }
    println!("RESULT ip={ip} ok={ok} ms={}", start.elapsed().as_millis());
    std::process::exit(if ok { 0 } else { 1 });
}
```


---

## 부록 G. `spikes/signaling`

`package.json` 의 버전은 설치 결과를 고정한 값이다. `package-lock.json` 과 `worker-configuration.d.ts` 는 명령으로 생성한다.

`spikes/signaling/package.json`:

```json
{
  "name": "cfspike",
  "private": true,
  "type": "module",
  "scripts": {
    "test": "vitest run",
    "typecheck": "tsc --noEmit"
  },
  "devDependencies": {
    "@cloudflare/vitest-plugin": "1.2.4",
    "@cloudflare/workers-types": "5.20260923.1",
    "typescript": "5.9.3",
    "vitest": "4.1.11",
    "wrangler": "4.137.0"
  }
}
```

`spikes/signaling/wrangler.jsonc`:

```jsonc
{
  "$schema": "node_modules/wrangler/config-schema.json",
  "name": "cfspike",
  "main": "src/index.ts",
  "compatibility_date": "2026-09-01",
  "durable_objects": {
    "bindings": [{ "name": "ROOM", "class_name": "Room" }]
  },
  "migrations": [{ "tag": "v1", "new_sqlite_classes": ["Room"] }]
}
```

`spikes/signaling/vitest.config.ts`:

```ts
import { cloudflareTest } from "@cloudflare/vitest-plugin";
import { defineConfig } from "vitest/config";

export default defineConfig({
	plugins: [cloudflareTest({ wrangler: { configPath: "./wrangler.jsonc" } })],
});
```

`spikes/signaling/tsconfig.json`:

```json
{
	"compilerOptions": {
		"target": "es2022",
		"module": "es2022",
		"moduleResolution": "bundler",
		"lib": ["es2022"],
		"strict": true,
		"noEmit": true,
		"skipLibCheck": true,
		"types": ["@cloudflare/workers-types", "@cloudflare/vitest-plugin/types"]
	},
	"include": ["src/**/*.ts", "test/**/*.ts", "worker-configuration.d.ts"]
}
```

`spikes/signaling/src/index.ts`:

```ts
import { DurableObject } from "cloudflare:workers";

export class Room extends DurableObject<Env> {
	fetch(request: Request): Response {
		if (request.headers.get("Upgrade") !== "websocket") {
			return new Response("expected websocket", { status: 426 });
		}
		const { 0: client, 1: server } = new WebSocketPair();
		// Hibernation API: the runtime owns the socket, DO may be evicted between messages.
		this.ctx.acceptWebSocket(server);
		return new Response(null, { status: 101, webSocket: client });
	}

	webSocketMessage(ws: WebSocket, message: string | ArrayBuffer): void {
		for (const other of this.ctx.getWebSockets()) {
			if (other !== ws) other.send(message);
		}
	}

	webSocketClose(ws: WebSocket, code: number, reason: string): void {
		ws.close(code, reason);
	}
}

export default {
	fetch(request, env) {
		const match = new URL(request.url).pathname.match(/^\/room\/([^/]+)$/);
		if (request.method !== "GET" || match === null) {
			return new Response("not found", { status: 404 });
		}
		const stub = env.ROOM.get(env.ROOM.idFromName(match[1]));
		return stub.fetch(request);
	},
} satisfies ExportedHandler<Env>;
```

`spikes/signaling/test/room.test.ts`:

```ts
import { runInDurableObject } from "cloudflare:test";
import { env, exports } from "cloudflare:workers";
import { expect, it } from "vitest";

async function connect(roomId: string): Promise<WebSocket> {
	const res = await exports.default.fetch(`https://example.com/room/${roomId}`, {
		headers: { Upgrade: "websocket" },
	});
	expect(res.status).toBe(101);
	const ws = res.webSocket;
	if (!ws) throw new Error("no webSocket on response");
	ws.accept();
	return ws;
}

function nextMessage(ws: WebSocket): Promise<string> {
	return new Promise((resolve, reject) => {
		const timer = setTimeout(() => reject(new Error("timeout")), 5_000);
		ws.addEventListener("message", (e) => {
			clearTimeout(timer);
			resolve(typeof e.data === "string" ? e.data : new TextDecoder().decode(e.data as ArrayBuffer));
		}, { once: true });
	});
}

it("relays a message from A to B in the same room", async () => {
	const a = await connect("r1");
	const b = await connect("r1");
	const received = nextMessage(b);
	a.send("hello");
	expect(await received).toBe("hello");
	a.close(1000, "done");
	b.close(1000, "done");
});

it("does not relay across rooms", async () => {
	const a = await connect("r2");
	const c = await connect("r3");
	let got = false;
	c.addEventListener("message", () => { got = true; });
	a.send("hello");
	await new Promise((r) => setTimeout(r, 200));
	expect(got).toBe(false);
});

it("rejects non-websocket requests", async () => {
	const res = await exports.default.fetch("https://example.com/room/r4");
	expect(res.status).toBe(426);
});

it("Room holds both accepted sockets in hibernation state", async () => {
	await connect("r5");
	await connect("r5");
	const stub = env.ROOM.get(env.ROOM.idFromName("r5"));
	const count = await runInDurableObject(stub, (_instance, state) => state.getWebSockets().length);
	expect(count).toBe(2);
});
```


---

## 부록 H. `spikes/egui-probe`

Rust 1.95 는 `spikes/rust-toolchain.toml` 이 적용한다.

`spikes/egui-probe/Cargo.toml`:

```toml
[package]
name = "egui_probe"
version = "0.1.0"
edition = "2024"

[dependencies]
eframe = "=0.36.2"
```

`spikes/egui-probe/src/main.rs`:

```rust
// Spike only (throwaway).
//! egui_probe: eframe 0.36.2 spike for a remote-desktop viewer.
//!
//! - Synthetic 1920x1080 RGBA frame, uploaded to one TextureHandle per new frame.
//! - Upload mode toggle: `set` (full delta, wgpu recreates the texture) vs
//!   `set_partial([0,0])` (reuses the texture, only `queue.write_texture`).
//! - Timing overlay: gen / convert / upload / whole ui() / eframe cpu_usage.
//! - Korean IME test fields and logs of Ime/Text and Key events.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use eframe::egui;
use egui::epaint::text::{FontInsert, FontPriority, InsertFontFamily};
use egui::{Color32, ColorImage, Event, ImeEvent, TextureHandle, TextureOptions};

const W: usize = 1920;
const H: usize = 1080;
const SAMPLES: usize = 240;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("egui_probe")
            .with_inner_size([1280.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "egui_probe",
        options,
        Box::new(|cc| Ok(Box::new(Probe::new(&cc.egui_ctx)))),
    )
}

/// Rolling window of millisecond samples.
#[derive(Default)]
struct Stat(VecDeque<f32>);

impl Stat {
    fn push(&mut self, ms: f32) {
        if self.0.len() == SAMPLES {
            self.0.pop_front();
        }
        self.0.push_back(ms);
    }

    fn summary(&self) -> String {
        if self.0.is_empty() {
            return "-".to_owned();
        }
        let mut v: Vec<f32> = self.0.iter().copied().collect();
        v.sort_by(f32::total_cmp);
        let avg = v.iter().sum::<f32>() / v.len() as f32;
        let p95 = v[((v.len() - 1) as f32 * 0.95).round() as usize];
        let max = v[v.len() - 1];
        format!("avg {avg:6.2}  p95 {p95:6.2}  max {max:6.2} ms")
    }
}

fn ms(d: Duration) -> f32 {
    d.as_secs_f32() * 1000.0
}

fn push_log(log: &mut VecDeque<String>, cap: usize, line: String) {
    if log.len() == cap {
        log.pop_front();
    }
    log.push_back(line);
}

struct Probe {
    tex: TextureHandle,
    rgba: Vec<u8>,
    frame_no: u64,
    pace_30fps: bool,
    use_set_partial: bool,
    next_frame_at: Instant,
    paint_times: VecDeque<f64>,
    gen_ms: Stat,
    convert_ms: Stat,
    upload_ms: Stat,
    ui_ms: Stat,
    cpu_usage_ms: Stat,
    single: String,
    multi: String,
    ime_log: VecDeque<String>,
    key_log: VecDeque<String>,
    font_note: String,
}

impl Probe {
    fn new(ctx: &egui::Context) -> Self {
        let font_note = install_korean_font(ctx);
        let rgba = vec![0u8; W * H * 4];
        let tex = ctx.load_texture(
            "video",
            ColorImage::from_rgba_unmultiplied([W, H], &rgba),
            TextureOptions::LINEAR,
        );
        Self {
            tex,
            rgba,
            frame_no: 0,
            pace_30fps: true,
            use_set_partial: true,
            next_frame_at: Instant::now(),
            paint_times: VecDeque::new(),
            gen_ms: Stat::default(),
            convert_ms: Stat::default(),
            upload_ms: Stat::default(),
            ui_ms: Stat::default(),
            cpu_usage_ms: Stat::default(),
            single: String::new(),
            multi: String::new(),
            ime_log: VecDeque::new(),
            key_log: VecDeque::new(),
            font_note,
        }
    }

    /// Moving gradient plus a white frame-counter bar across the top 40 rows.
    fn generate(&mut self) {
        let n = self.frame_no as usize;
        let bar = n % W;
        let blue = (n * 3 % 256) as u8;
        for (y, row) in self.rgba.chunks_exact_mut(W * 4).enumerate() {
            let g = ((y + n) & 0xff) as u8;
            for (x, px) in row.chunks_exact_mut(4).enumerate() {
                let white = y < 40 && x <= bar;
                if white {
                    px.copy_from_slice(&[255, 255, 255, 255]);
                } else {
                    px.copy_from_slice(&[((x + 2 * n) & 0xff) as u8, g, blue, 255]);
                }
            }
        }
    }

    fn produce_frame(&mut self) {
        let t0 = Instant::now();
        self.generate();
        let t1 = Instant::now();
        let image = ColorImage::from_rgba_unmultiplied([W, H], &self.rgba);
        let t2 = Instant::now();
        if self.use_set_partial {
            self.tex.set_partial([0, 0], image, TextureOptions::LINEAR);
        } else {
            self.tex.set(image, TextureOptions::LINEAR);
        }
        let t3 = Instant::now();
        self.gen_ms.push(ms(t1 - t0));
        self.convert_ms.push(ms(t2 - t1));
        self.upload_ms.push(ms(t3 - t2));
        self.frame_no += 1;
    }

    fn collect_events(&mut self, ctx: &egui::Context) {
        let (time, events) = ctx.input(|i| (i.time, i.events.clone()));
        for ev in events {
            match ev {
                Event::Ime(ime) => {
                    let line = match ime {
                        ImeEvent::Preedit {
                            text,
                            active_range_chars,
                        } => format!("Preedit({text:?}, {active_range_chars:?})"),
                        ImeEvent::Commit(text) => format!("Commit({text:?})"),
                        ImeEvent::DeleteSurrounding {
                            before_chars,
                            after_chars,
                        } => format!("DeleteSurrounding({before_chars}, {after_chars})"),
                        #[expect(deprecated)]
                        ImeEvent::Enabled => "Enabled".to_owned(),
                        #[expect(deprecated)]
                        ImeEvent::Disabled => "Disabled".to_owned(),
                    };
                    push_log(&mut self.ime_log, 20, format!("{time:9.3} Ime {line}"));
                }
                Event::Text(text) => {
                    push_log(&mut self.ime_log, 20, format!("{time:9.3} Text({text:?})"));
                }
                Event::Key {
                    key,
                    physical_key,
                    pressed,
                    repeat,
                    modifiers,
                } => {
                    push_log(
                        &mut self.key_log,
                        10,
                        format!(
                            "{time:9.3} {key:?} phys={physical_key:?} pressed={pressed} repeat={repeat} mods={modifiers:?}"
                        ),
                    );
                }
                _ => {}
            }
        }
    }
}

/// egui's default fonts have no Hangul glyphs, so load a system font.
fn install_korean_font(ctx: &egui::Context) -> String {
    let candidates = [
        r"C:\Windows\Fonts\malgun.ttf",
        "/usr/share/fonts/truetype/nanum/NanumGothic.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            ctx.add_font(FontInsert::new(
                "korean",
                egui::FontData::from_owned(bytes),
                vec![
                    InsertFontFamily {
                        family: egui::FontFamily::Proportional,
                        priority: FontPriority::Lowest,
                    },
                    InsertFontFamily {
                        family: egui::FontFamily::Monospace,
                        priority: FontPriority::Lowest,
                    },
                ],
            ));
            return format!("Korean font: {path}");
        }
    }
    "Korean font: NOT FOUND (Hangul will render as boxes)".to_owned()
}

impl eframe::App for Probe {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ui_start = Instant::now();
        let ctx = ui.ctx().clone();

        if let Some(cpu) = frame.info().cpu_usage {
            self.cpu_usage_ms.push(cpu * 1000.0);
        }

        let now_t = ctx.input(|i| i.time);
        self.paint_times.push_back(now_t);
        while self.paint_times.front().is_some_and(|t| now_t - t > 1.0) {
            self.paint_times.pop_front();
        }

        self.collect_events(&ctx);

        let now = Instant::now();
        if !self.pace_30fps || now >= self.next_frame_at {
            self.produce_frame();
            self.next_frame_at = if now > self.next_frame_at + Duration::from_millis(100) {
                now + Duration::from_secs_f64(1.0 / 30.0)
            } else {
                self.next_frame_at + Duration::from_secs_f64(1.0 / 30.0)
            };
        }
        if self.pace_30fps {
            ctx.request_repaint_after(self.next_frame_at.saturating_duration_since(Instant::now()));
        } else {
            ctx.request_repaint();
        }

        egui::Panel::right("side")
            .resizable(true)
            .default_size(460.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let (ppp, native_ppp, dt) = ctx.input(|i| {
                        (i.pixels_per_point, i.viewport().native_pixels_per_point, i.unstable_dt)
                    });
                    ui.monospace(format!("paints/s  {}", self.paint_times.len()));
                    ui.monospace(format!("video frames  {}", self.frame_no));
                    ui.monospace(format!("unstable_dt  {:.2} ms", dt * 1000.0));
                    ui.monospace(format!(
                        "pixels_per_point {ppp:.3}  native {native_ppp:?}"
                    ));
                    ui.monospace(format!("gen      {}", self.gen_ms.summary()));
                    ui.monospace(format!("convert  {}", self.convert_ms.summary()));
                    ui.monospace(format!("upload   {}", self.upload_ms.summary()));
                    ui.monospace(format!("ui()     {}", self.ui_ms.summary()));
                    ui.monospace(format!("cpu_usage{}", self.cpu_usage_ms.summary()));
                    ui.label("(cpu_usage = eframe's previous-frame ui()+render, vsync excluded on wgpu)");
                    ui.checkbox(&mut self.pace_30fps, "pace video at 30 fps (else every repaint)");
                    ui.checkbox(
                        &mut self.use_set_partial,
                        "upload with set_partial([0,0]) (else set = new GPU texture)",
                    );
                    ui.label(&self.font_note);
                    ui.separator();

                    ui.label("single-line:");
                    ui.add(egui::TextEdit::singleline(&mut self.single).desired_width(f32::INFINITY));
                    ui.label("multi-line:");
                    ui.add(
                        egui::TextEdit::multiline(&mut self.multi)
                            .desired_rows(4)
                            .desired_width(f32::INFINITY),
                    );
                    ui.separator();

                    ui.label("last 20 Ime/Text events:");
                    for line in &self.ime_log {
                        ui.monospace(line);
                    }
                    ui.separator();
                    ui.label("last 10 Key events:");
                    for line in &self.key_log {
                        ui.monospace(line);
                    }
                });
            });

        egui::CentralPanel::no_frame().show(ui, |ui| {
            let avail = ui.available_rect_before_wrap();
            let scale = (avail.width() / W as f32).min(avail.height() / H as f32);
            let size = egui::vec2(W as f32 * scale, H as f32 * scale);
            let rect = egui::Rect::from_center_size(avail.center(), size);
            ui.painter().rect_filled(avail, 0.0, Color32::BLACK);
            egui::Image::from_texture(&self.tex).paint_at(ui, rect);
        });

        self.ui_ms.push(ms(ui_start.elapsed()));
    }
}
```

---

## 부록 I. `spikes/wix`

`spikes/wix/service/Cargo.toml`:

```toml
[package]
name = "spike-service"
version = "0.1.0"
edition = "2024"

[dependencies]

[target.'cfg(windows)'.dependencies]
windows-service = "=0.8.1"
```

`spikes/wix/service/src/main.rs`:

```rust
//! Spike only (throwaway): minimal LocalSystem service used to verify the WiX MSI
//! (service install, failure restart, ProgramData ACL). Not product code.
//!
//! Behaviour
//! - Appends "<unix-seconds> running pid=<pid>" to C:\ProgramData\SimpleRemoteSpike\service.log every 5 s.
//! - If C:\ProgramData\SimpleRemoteSpike\crash-once exists at start, deletes it and
//!   exits the process with code 1 after 2 s without reporting Stopped (simulated crash).

#[cfg(windows)]
fn main() -> windows_service::Result<()> {
    spike::run()
}

#[cfg(not(windows))]
fn main() {
    eprintln!("windows only");
}

#[cfg(windows)]
mod spike {
    use std::ffi::OsString;
    use std::io::Write;
    use std::path::Path;
    use std::sync::mpsc;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use windows_service::service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    };
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
    use windows_service::{define_windows_service, service_dispatcher, Result};

    const SERVICE_NAME: &str = "SimpleRemoteSpike";
    const DATA_DIR: &str = r"C:\ProgramData\SimpleRemoteSpike";

    pub fn run() -> Result<()> {
        service_dispatcher::start(SERVICE_NAME, ffi_service_main)
    }

    define_windows_service!(ffi_service_main, service_main);

    fn service_main(_args: Vec<OsString>) {
        let _ = run_service();
    }

    fn log(line: &str) {
        let path = Path::new(DATA_DIR).join("service.log");
        // The MSI creates DATA_DIR; the service does not create it.
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let _ = writeln!(f, "{now} {line} pid={}", std::process::id());
        }
    }

    fn status(state: ServiceState, accept: ServiceControlAccept) -> ServiceStatus {
        ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: state,
            controls_accepted: accept,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        }
    }

    fn run_service() -> Result<()> {
        let (tx, rx) = mpsc::channel();
        let handler = move |event| -> ServiceControlHandlerResult {
            match event {
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                ServiceControl::Stop => {
                    let _ = tx.send(());
                    ServiceControlHandlerResult::NoError
                }
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        };
        let handle = service_control_handler::register(SERVICE_NAME, handler)?;
        handle.set_service_status(status(ServiceState::Running, ServiceControlAccept::STOP))?;
        log("started");

        let marker = Path::new(DATA_DIR).join("crash-once");
        if marker.exists() {
            let _ = std::fs::remove_file(&marker);
            log("crash-once marker found, exiting with code 1");
            std::thread::sleep(Duration::from_secs(2));
            std::process::exit(1);
        }

        loop {
            match rx.recv_timeout(Duration::from_secs(5)) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => log("running"),
            }
        }
        log("stopping");
        handle.set_service_status(status(ServiceState::Stopped, ServiceControlAccept::empty()))?;
        Ok(())
    }
}
```

`spikes/wix/Spike.wxs`:

```xml
<!-- Spike only (throwaway): WiX 7 MSI for SP1 Phase 0 installer checks. -->
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs"
     xmlns:util="http://wixtoolset.org/schemas/v4/wxs/util"
     xmlns:fw="http://wixtoolset.org/schemas/v4/wxs/firewall">
  <Package Id="SimpleRemote.Spike"
           Name="SimpleRemote Spike"
           Manufacturer="SimpleRemote"
           Version="0.0.1"
           Scope="perMachine">

    <MediaTemplate EmbedCab="yes" />

    <!-- RemoveFolderEx needs a property that resolves before CostInitialize; Directory ids cannot be used.
         %ProgramData% is expanded by the custom action (RemoveFolderEx.xsd, Property attribute). -->
    <Property Id="SPIKEDATADIR" Value="%ProgramData%\SimpleRemoteSpike" />

    <StandardDirectory Id="ProgramFiles64Folder">
      <Directory Id="INSTALLFOLDER" Name="SimpleRemoteSpike">
        <Component Id="SpikeServiceComponent">
          <File Id="SpikeExe" Source="spike-service.exe" KeyPath="yes">
            <fw:FirewallException Id="SpikeFirewall"
                                  Name="SimpleRemote Spike"
                                  Scope="any"
                                  Profile="all" />
          </File>
          <ServiceInstall Id="SpikeService"
                          Name="SimpleRemoteSpike"
                          DisplayName="SimpleRemote Spike"
                          Description="SimpleRemote installer spike service"
                          Type="ownProcess"
                          Start="auto"
                          ErrorControl="normal"
                          Account="LocalSystem"
                          Vital="yes">
            <util:ServiceConfig FirstFailureActionType="restart"
                                SecondFailureActionType="restart"
                                ThirdFailureActionType="restart"
                                RestartServiceDelayInSeconds="5"
                                ResetPeriodInDays="1" />
          </ServiceInstall>
          <ServiceControl Id="SpikeServiceControl"
                          Name="SimpleRemoteSpike"
                          Start="install"
                          Stop="both"
                          Remove="uninstall"
                          Wait="yes" />
        </Component>
      </Directory>
    </StandardDirectory>

    <StandardDirectory Id="CommonAppDataFolder">
      <Directory Id="DATAFOLDER" Name="SimpleRemoteSpike">
        <!-- Directory key path: Guid cannot be auto-generated (Compiler.cs IllegalComponentWithAutoGeneratedGuid). -->
        <Component Id="SpikeDataFolderComponent" Guid="6B0F5C1E-2D4A-4E7B-9C3D-1A2B3C4D5E6F">
          <CreateFolder>
            <!-- Protected DACL (P), inheritable (OICI) Full Access (FA) for LocalSystem (SY) only. -->
            <PermissionEx Sddl="D:P(A;OICI;FA;;;SY)" />
          </CreateFolder>
          <util:RemoveFolderEx Property="SPIKEDATADIR" On="uninstall" />
        </Component>
      </Directory>
    </StandardDirectory>

    <Feature Id="Main">
      <ComponentRef Id="SpikeServiceComponent" />
      <ComponentRef Id="SpikeDataFolderComponent" />
    </Feature>
  </Package>
</Wix>
```

`spikes/wix/check.ps1`:

```powershell
# Spike only (throwaway): install the spike MSI, check the real state, uninstall, check traces.
# Prints "CHECK <name> ok=<bool>" per item and "RESULT wix ok=<bool>" last.
param(
    [Parameter(Mandatory)] [string] $Msi,
    [Parameter(Mandatory)] [string] $PsExec
)
$ErrorActionPreference = 'Continue'
$name = 'SimpleRemoteSpike'
$exe = 'C:\Program Files\SimpleRemoteSpike\spike-service.exe'
$data = 'C:\ProgramData\SimpleRemoteSpike'
$results = [ordered]@{}

function Check([string] $label, [bool] $ok, [string] $detail = '') {
    $results[$label] = $ok
    Write-Output "CHECK $label ok=$($ok.ToString().ToLower()) $detail"
}

# Read a file inside the SYSTEM-only folder as SYSTEM.
function Read-AsSystem([string] $path) {
    & $PsExec -accepteula -nobanner -s cmd /c type "$path" 2>$null | Out-String
}

$p = Start-Process msiexec.exe -ArgumentList "/i `"$Msi`" /qn /l*v install.log" -Wait -PassThru
Check 'install-exit-code' ($p.ExitCode -eq 0) "code=$($p.ExitCode)"

$svc = Get-CimInstance Win32_Service -Filter "Name='$name'"
Check 'service-exists' ($null -ne $svc)
Check 'service-account-localsystem' ($svc.StartName -eq 'LocalSystem') "StartName=$($svc.StartName)"
Check 'service-auto-start' ($svc.StartMode -eq 'Auto') "StartMode=$($svc.StartMode)"
Check 'service-running' ($svc.State -eq 'Running') "State=$($svc.State)"

$qf = sc.exe qfailure $name | Out-String
Write-Output $qf
Check 'service-failure-restart' ($qf -match 'RESTART') ''

$rule = Get-NetFirewallRule -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -eq 'SimpleRemote Spike' }
$ruleProgram = if ($rule) { ($rule | Get-NetFirewallApplicationFilter).Program } else { '' }
Check 'firewall-rule-program' ($null -ne $rule -and $ruleProgram -eq $exe) "Program=$ruleProgram"

icacls $data | Write-Output
$acl = Get-Acl $data -ErrorAction SilentlyContinue
$ids = if ($acl) { @($acl.Access | ForEach-Object { $_.IdentityReference.Value } | Sort-Object -Unique) } else { @() }
Check 'data-acl-system-only' ($acl -and $acl.AreAccessRulesProtected -and $ids.Count -eq 1 -and $ids[0] -eq 'NT AUTHORITY\SYSTEM') "ids=$($ids -join ';')"
$adminRead = Test-Path (Join-Path $data 'service.log') -ErrorAction SilentlyContinue
Write-Output "admin can see service.log: $adminRead"

Start-Sleep -Seconds 12
$log1 = Read-AsSystem (Join-Path $data 'service.log')
Write-Output $log1
Check 'service-writes-log' ($log1 -match 'running') ''

# Simulated crash: marker makes the next start exit with code 1 without reporting Stopped.
& $PsExec -accepteula -nobanner -s cmd /c "echo x> `"$data\crash-once`"" 2>$null
Restart-Service $name -Force
Start-Sleep -Seconds 20
$log2 = Read-AsSystem (Join-Path $data 'service.log')
Write-Output $log2
$pids = @([regex]::Matches($log2, 'started pid=(\d+)') | ForEach-Object { $_.Groups[1].Value } | Select-Object -Unique)
$svc2 = Get-CimInstance Win32_Service -Filter "Name='$name'"
Check 'service-restarted-after-crash' (($log2 -match 'crash-once marker found') -and $pids.Count -ge 3 -and $svc2.State -eq 'Running') "started_pids=$($pids -join ',') State=$($svc2.State)"

$u = Start-Process msiexec.exe -ArgumentList "/x `"$Msi`" /qn /l*v uninstall.log" -Wait -PassThru
Check 'uninstall-exit-code' ($u.ExitCode -eq 0) "code=$($u.ExitCode)"
Check 'trace-service-gone' ($null -eq (Get-CimInstance Win32_Service -Filter "Name='$name'"))
Check 'trace-programfiles-gone' (-not (Test-Path 'C:\Program Files\SimpleRemoteSpike'))
Check 'trace-programdata-gone' (-not (Test-Path $data))
Check 'trace-firewall-gone' ($null -eq (Get-NetFirewallRule -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -eq 'SimpleRemote Spike' }))

$all = -not ($results.Values -contains $false)
Write-Output "RESULT wix ok=$($all.ToString().ToLower())"
if (-not $all) { exit 1 }
```
