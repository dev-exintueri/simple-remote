# SP1 Phase 0 spike 결과

계획: `docs/superpowers/plans/2026-09-23-sp1-phase0-spikes.md`. 각 절은 해당 task 가 채운다.
형식: 질문 / 기준 / 실행 환경(OS, 버전, 명령) / 출력 요약 / 판정(통과·실패·부분) / 설계 영향.

## T1 GitHub Actions Windows 빌드

- 질문: Actions 가 켜져 있나? `windows-2025` 에서 Rust spike 를 빌드·실행할 수 있나? 한 번에 몇 분 걸리나?
- 기준: workflow 실행, `winprobe` 빌드, runner 계정 DPAPI 왕복 성공, artifact 업로드.
- 실행: https://github.com/dev-exintueri/simple-remote/actions/runs/35894682472 (commit `7fcd83e`)
- 출력 요약
  - job 전체 1분 21초. toolchain 1.95 설치 14초, `win-probes` release 빌드(cache 없음) 45초.
  - `user=runneradmin session=2`. DPAPI 왕복 `plaintext=spike-secret`, `dpapi roundtrip RESULT ok=true`.
  - `mf_probe`: H.264 인코더 1개 `H264 Encoder MFT (hardware=false)`. 하드웨어 모드는 인코더 없음(GPU 없음, 예상대로). 소프트웨어 모드 `frames_in=30 frames_out=27 bytes_out=19753 first_output_ms=62`. runner(Windows Server 2025)에 Media Foundation 이 있다.
  - artifact `win-spikes` (exe 3개, 234 KB) 업로드 성공.
- 판정: **통과**.
- 사용량 계산: 이 job 은 분 단위 올림으로 2분, Windows 2배 차감으로 4분. WebRTC 테스트와 WiX 가 더해진 뒤의 시간은 T6, T13 에서 다시 잰다. 이 크기면 한 달 1,000분(Windows 기준)에서 약 250회.
- 참고: runner 는 session 2 의 대화형 세션에서 돈다 (session 0 서비스가 아님). T10 의 SendInput 도 runner 에서 해 볼 수 있으나 모니터 구성이 하나뿐이라 사람 PC 확인을 대신하지 못한다.

## T2 PAKE

- 질문: RFC 9382 테스트 값을 재현하는 Rust SPAKE2 구현이 있나? 틀린 코드를 key 확인 단계에서 거부하나? spec 5.7 의 후보 `spake2` 0.4.0 은 RFC 9382 를 따르나?
- 기준: `pakery_rfc9382` 3건 통과 (RFC 9382 Appendix B vector 1 전체 일치 + 틀린 코드 `ConfirmationFailed`).
- 실행 환경: Linux x86_64 (클라우드), rustc 1.95.0, `cd spikes/auth && cargo test --test pakery_rfc9382 --test spake2_crate`
- 출력 요약: `pakery_rfc9382` 3 passed, `spake2_crate` 5 passed.
- 판정: **통과**. `pakery-spake2` 0.6.0 + `pakery-crypto` 0.6.0 (P256-SHA256-HKDF-SHA256-HMAC-SHA256) 이 vector 1 의 pA, pB, K, Hash(TT), Ke, KcA, KcB, MAC_A, MAC_B 를 모두 재현했고, 틀린 코드는 양쪽 `verify_peer_confirmation` 에서 `ConfirmationFailed` 로 거부됐다.

| 항목 | `pakery-spake2` 0.6.0 | `spake2` 0.4.0 |
|---|---|---|
| 표준 | RFC 9382 (transcript, key schedule, key 확인 MAC) | draft-ladd-spake2-01 (RFC 이전), python-spake2 호환 |
| 곡선 | P-256 (RFC suite). Ristretto255 suite 는 RFC 가 아니라고 스스로 표시 | Ed25519 만 |
| key 확인 | 있음 (constant-time 비교) | 없음. 틀린 코드에서도 `finish` 가 성공하고 다른 key 를 낸다 (`spake2_crate` 테스트로 고정) |
| 출력 key | Ke 16 byte | 32 byte |
| 감사 | 없음 (README 명시) | 없음 (spec 5.7) |
| 관리 | 1인 프로젝트, GitHub 별 2, 2026-02 시작, 2026-09 에 0.3 → 0.6 (호환 깨짐 잦음), `forbid(unsafe_code)` | RustCrypto 조직 |
| 의존성 세대 | `rand_core` 0.10, `sha2` 0.11 | `rand_core` 0.6, `sha2` 0.10 |

- `opaque-ke` 4.0.1: 서버에 비밀번호 등록 기록을 두는 aPAKE 다 (`src/opaque.rs` 등록 2단계 + 로그인 3단계). host 가 매 세션 새 코드를 만드는 구조에서는 매번 자기 자신에게 등록을 해야 하고, 등록 기록 유출 보호라는 aPAKE 의 장점이 일회용 코드에는 의미가 없다. 코드 비교 없이 제외.
- 의존성: 한 package 안에 `rand_core` 0.6/0.10, `sha2` 0.10/0.11, `digest` 0.10/0.11 이 같이 들어가도 빌드된다 (`cargo tree -d`). 대신 RNG 는 각 세대 trait 에 맞게 따로 만든다.
- 설계 영향 (Task 14 에서 spec 반영 승인 요청)
  - spec 5.2 의 "SPAKE2, RFC 9382 후보"와 "key 확인 MAC 까지 끝나야 다음으로" 를 그대로 만족하는 것은 `pakery-spake2` 뿐이다.
  - 권고: `pakery-spake2` P-256 suite 를 쓰고 `crates/auth` 안에 격리, 버전을 `=0.6.0` 으로 고정. 근거: spec 요구(RFC 9382, key 확인)를 코드 추가 없이 만족하고 RFC 테스트 값으로 검증된다. 위험(1인 프로젝트, 미감사, 잦은 API 변경)은 버전 고정과 RFC vector 테스트를 `crates/auth` 테스트에 두는 것으로 줄인다.
  - 대안: `spake2` 0.4 + key 확인 MAC 직접 구현. RFC 9382 가 아니므로 spec 문구를 바꿔야 하고, 직접 만든 암호 코드가 생긴다.

## T3 재접속 key 합의 (Noise KK)

- 질문: `snow` 로 Noise KK 상호 인증 key 합의가 되나? 상대 key 가 틀리면 첫 메시지에서 실패하나? Ed25519 기기 key 를 그대로 쓸 수 있나?
- 기준: `noise_kk` 3건 통과.
- 실행 환경: Linux x86_64 (클라우드), rustc 1.95.0, `snow` 0.10.0, `Noise_KK_25519_ChaChaPoly_BLAKE2s`, `cd spikes/auth && cargo test --test noise_kk`
- 출력 요약: 3 passed (왕복 후 transport 양방향, 틀린 responder key → `Error::Decrypt`, 틀린 initiator key → `Error::Decrypt`). 첫 메시지는 빈 payload 여도 48 byte (e 32 + tag 16) 라 틀린 key 는 첫 메시지에서 걸린다.
- 판정: **통과**.
- key 종류: `snow` 의 25519 는 X25519 (Montgomery) 뿐이고 Ed25519 key 를 받지 않는다 (`resolvers/default.rs` `Dh25519`). 또 key 길이를 검사하지 않는다 (짧은 private key 는 0 으로 채움, `default.rs:220-225`).
- 설계 영향 (Task 14 에서 spec 반영 승인 요청)
  - 권고: 기기마다 재접속용 X25519 정적 key 를 하나 더 두고, 공개키를 Ed25519 key 로 서명해 기기 신원에 묶는다 (spec 4절 표에 행 추가). 근거: Ed25519 → X25519 변환 코드를 우리가 다루지 않아도 되고, 저장·폐기 규칙은 Ed25519 key 와 같게 둘 수 있다.
  - `crates/auth` 는 `snow` 에 넘기기 전에 key 길이가 정확히 32 byte 인지 확인한다.

## T4 WebRTC: str0m

- 질문: 외부 후보, ICE restart, H.264 RTP, data channel 신뢰·비신뢰, 대역폭 추정 (계획 Task 4 의 5개).
- 기준: 테스트 파일 5개(6건) 통과.
- 실행 환경: Linux x86_64 (클라우드), rustc 1.95.0, str0m 0.23.1 (`aws-lc-rs`), 시간을 흉내 내는 메모리 안 가짜 네트워크 (socket 없음). `cd spikes/webrtc-str0m && cargo test -- --nocapture --test-threads=1` (실제 소요 약 1.5초)
- 출력 요약: 6 passed.

| 질문 | 판정 | 측정값 |
|---|---|---|
| 1. 외부 후보 | 통과 | srflx(127.0.0.2:40000) 만 SDP 로 알리고 NAT 흉내를 거쳐 60ms 에 연결. 선택 경로는 `PeerStats` 로 확인 (L: remote=127.0.0.2:40000). local 에 같은 socket 의 host 후보가 없으면 10초 안에 연결 안 됨 (소스 읽기 결과를 실행으로 확인) |
| 2. ICE restart | 통과 | viewer 주소를 바꾸고 옛 주소를 막은 뒤 str0m 이 스스로 ICE Disconnected 를 내기까지 21.3초. restart 시작부터 ICE 연결 60ms, 첫 데이터 20ms. 같은 `ChannelId` 가 양방향으로 계속 동작 |
| 3. H.264 | 통과 | Annex-B frame 6개(IDR 5039 byte = RTP 6개, P 3005 byte) 가 byte 단위로 같게 도착, keyframe 표시 정확. 받는 쪽 PLI 가 보내는 쪽 `Event::KeyframeRequest` 로 10ms 에 도착 |
| 4. data channel | 통과 | 연결 뒤 양방향 5번째마다 손실(약 20%). 신뢰·순서 channel 200/200 순서대로 (마지막 도착 8.26초), 비신뢰(`MaxRetransmits 0`, unordered) 168/200, 중복 없음 |
| 5. 대역폭 추정 | 통과 | 병목 1 Mbps 20초: 추정 최고 1.139 Mbps, 끝값 0.926 Mbps (queue 지연 최대 약 390ms). 5 Mbps 로 풀린 뒤 4.96초에 1.5 Mbps 초과, 약 8초 뒤 4.92 Mbps, 20초 뒤 6.13 Mbps |

- 판정: **통과 (5/5)**.
- 설계 영향
  - ICE 자체의 끊김 감지는 21초로 spec 8.3 의 5초 ICE restart 보다 훨씬 늦다. 앱 heartbeat(500ms)로 끊김을 판단하고 restart 를 직접 시작하는 spec 설계가 필요함을 확인했다. 라이브러리 감지에 기대지 않는다.
  - UPnP 매핑 후보는 SDP 에 srflx 로 알리되, local 에는 같은 socket 의 host 후보를 반드시 함께 둔다 (spec 5.4 반영 후보).
  - 경로 기록(spec 5.4)은 `RtcConfig::set_stats_interval` 의 `Event::PeerStats.selected_candidate_pair` 로 만든다. local 주소는 srflx 가 아니라 base socket 주소로 나온다.
  - 병목 1 Mbps 에서 queue 지연이 수백 ms 까지 쌓였다. 1초당 1회 추정값을 encoder bitrate 로 넘기는 이 테스트의 단순한 방식으로는 지연이 커질 수 있다. 제품의 bitrate 조절 주기와 FPS 낮추기(spec 6.2)는 codec 계획에서 실측으로 정한다.
  - 한계: 가짜 네트워크의 지연은 몇 ms 수준이라 시간 값은 실제 인터넷보다 짧다.

## T5 WebRTC: webrtc-rs

- 질문: T4 와 같은 5개.
- 기준: 테스트 파일 5개 통과.
- 실행 환경: Linux x86_64 (클라우드), rustc 1.95.0, webrtc/rtc 0.21.0, tokio multi_thread, 실제 loopback UDP socket + 손실·병목을 넣는 UDP 중계 task. `cd spikes/webrtc-rs && cargo test --no-fail-fast -- --nocapture --test-threads=1` (bwe 41초, 나머지 4개 파일 20초)
- 출력 요약: 8건 중 7 passed, `bwe` 1 failed.

| 질문 | 판정 | 측정값 |
|---|---|---|
| 1. 외부 후보 | 통과 | SDP 의 후보를 지우고 srflx 줄을 넣는 방식으로 812ms 에 연결. offerer 선택 경로 `127.0.0.2:40000 (srflx)`, answerer 는 상대를 prflx 로 앎 |
| 2. ICE restart | 통과 | `RTCOfferOptions{ice_restart}`, `restart_ice()`, socket 재bind 3가지 모두 ufrag 교체 후 약 2ms 에 재연결, 같은 DataChannel 로 왕복 약 5ms. viewer 주소 변경 후 끊김 감지 시간은 이 테스트에 없음 |
| 3. H.264 | 통과 | FU-A 조각 재조립이 byte 단위로 같음. PLI 는 직접 만든 interceptor 로 offerer 앱에 2건 도착. SPS/PPS STAP-A 가 MTU 를 넘으면 조용히 버려지는 동작을 unit test 로 확인 |
| 4. data channel | 통과 | 5번째마다 손실. 신뢰 200/200 순서대로 (11.05초), 비신뢰 152 수신. 단 answerer 쪽에서 본 비신뢰 channel 설정은 `max_retransmits=None` 으로 보고됨 (보낸 쪽 설정은 `Some(0)`). 동작은 비신뢰였으나 설정 보고가 다름 |
| 5. 대역폭 추정 | **실패** | 병목 1 Mbps: 추정이 1.52 → 0.22 Mbps 로 계속 하락. 5 Mbps 로 풀어도 0.19 → 0.12 Mbps 로 계속 하락 (`phase 2 estimate increases` assertion 실패) |

- 5번 실패의 원인 확인 (harness 인지 라이브러리인지)
  - 중계 task 의 병목 흉내가 받은 양을 그대로 내보낸다: 진단 실행에서 2초 이후 들어온 양과 내보낸 양이 같고(예: 0.41/0.41 Mbps) 버림 0. 패킷은 answerer socket 까지 간다.
  - 병목 없는 대조 실행: 보낸 2.59 Mbps 중 2.51 Mbps 가 받는 앱에 도착하고 추정이 초당 약 8% 오른다. 받는 경로와 GCC 는 손실이 없으면 동작한다.
  - 병목 실행: 첫 1초에 병목을 넘는 양(2.37 Mbps)을 보내 126개가 버려진 뒤부터, answerer socket 에 도착한 RTP 의 일부만(초 단위로 0~50%, 몇 초마다 몰아서) 받는 앱에 전달되고 추정이 계속 내려간다.
  - 결론: 손실이 한 번 생긴 뒤 webrtc-rs 받는 쪽 경로 또는 GCC 가 회복하지 못한다. 기본 interceptor 에 jitter buffer 는 없다 (`register_default_interceptors`: NACK, simulcast header, TWCC receiver, RTCP report). 라이브러리 안의 정확한 원인은 찾지 않았다. 선택 결과(T7)가 이미 갈려 더 파는 비용(시간)이 결정에 영향을 주지 않기 때문이다.
- 판정: **4/5 통과, 대역폭 추정 실패**.
- 소스 확인 결과와 합친 우회 필요 사항: local 후보 추가 API 없음(SDP 문자열 편집), PLI 수신에 interceptor 필요, GCC 기본 꺼짐과 추정값 꺼내는 API 없음, SPS/PPS 크기 확인 필요, mDNS 5353 bind 실패가 연결 실패가 됨.

## T6 WebRTC: Windows 빌드와 IPv6
## T7 WebRTC 라이브러리 결정
## T8 Cloudflare Workers 로컬 테스트

- 질문: Cloudflare 계정 없이 Durable Object + WebSocket(Hibernation API)을 로컬 테스트할 수 있나?
- 기준: 테스트 4건 통과, `tsc --noEmit` 통과, 로그인 없이.
- 실행 환경: Linux x86_64 (클라우드), Node 22.22.2, npm 11 (`npx -y npm@11 install`), `@cloudflare/vitest-plugin` 1.2.4, vitest 4.1.11, wrangler 4.137.0, workerd 1.20260921.1, `@cloudflare/workers-types` 5.20260923.1, typescript 5.9.3.
- 출력 요약: 구현 전 실행은 `src/index.ts` 를 찾지 못해 실패. 구현 후 `Tests 4 passed (4)` (같은 방 전달 18ms, 다른 방으로 새지 않음, WebSocket 아닌 요청 426, DO 안 socket 2개), `tsc --noEmit` 출력 없음. `~/.wrangler` 가 없고 로그인·token 을 쓰지 않았다.
- 판정: **통과**.
- 설계 영향과 주의
  - 테스트 도구 이름은 `@cloudflare/vitest-plugin` (1.0 에서 `@cloudflare/vitest-pool-workers` 가 바뀜). 설정은 `vitest/config` 의 `defineConfig` + `cloudflareTest({ wrangler: { configPath } })` plugin. vitest 5 는 peerDependency 밖이라 4.1 을 쓴다.
  - `cloudflare:test` 의 `SELF`, `env` 는 deprecated. `cloudflare:workers` 의 `exports.default.fetch`, `env` 를 쓴다.
  - Node 22 에 딸린 npm 10.9.7 은 이 의존성 조합에서 설치에 실패한다 (`edgesOut` null). npm 11 을 쓴다. npm 11 은 esbuild, workerd 의 postinstall 을 승인 없이 실행하지 않는데, 플랫폼별 binary package 가 따로 설치되어 테스트는 동작한다.
  - Durable Object 는 SQLite 기반(`new_sqlite_classes`)으로 선언했다. 무료 plan 에서 쓸 수 있는 방식인지는 배포 계획(signaling 계획)에서 Cloudflare 문서로 확인한다 (이 컨테이너는 developers.cloudflare.com 이 막혀 있음).

## T9 SYSTEM 계정 DPAPI
## T10 SendInput 절대 좌표
## T11 MF 하드웨어 인코더 (SYSTEM agent)
## T12 egui
## T13 WiX MSI
## 종합
