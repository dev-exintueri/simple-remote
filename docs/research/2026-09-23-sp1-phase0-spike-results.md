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

- 질문: 두 라이브러리가 MSVC 로 빌드되나? Windows 에서도 테스트 결과가 같나? Windows 에서 IPv6 host 후보로 실제 UDP 연결이 되나?
- 실행: https://github.com/dev-exintueri/simple-remote/actions/runs/35896459536 (commit `7c410cf`, windows-2025)
- 출력 요약
  - 빌드: 두 package 모두 MSVC 빌드 성공 (aws-lc-sys, ring 의 C 코드 포함). 테스트 빌드 시간(cache 없음) str0m 2분 45초, webrtc-rs 2분 36초, `ipv6-pair` release 4분 29초.
  - str0m: 6 passed. 측정값이 Linux 와 같다 (시간을 흉내 내는 가짜 네트워크라 결정적).
  - webrtc-rs: Linux 와 같게 `bwe` 만 실패 (phase1 끝값 0.22 Mbps, phase2 최대 0.18 Mbps), 나머지 통과. 중계 socket 의 `127.0.0.2` bind 가 Windows 에서도 된다 (계획의 확인할 가정 해소). 신뢰 channel 200건 도착 시간은 17.5초로 Linux(11.05초)보다 길었다.
  - IPv6: `RESULT ip=::1 ok=true ms=484` (실제 UDP socket, str0m).
- 클라우드 확인: 이 클라우드 컨테이너는 IPv6 가 없어 `::1` bind 가 `Address family not supported` 로 실패한다. 대신 `127.0.0.1` 로 `ok=true ms=213` 을 확인했다 (계획 2단계의 기대값과 다른 점).
- 판정: **Actions 부분 통과**. 사람 PC 의 전역 IPv6 주소 확인은 H1 답을 받은 뒤 한다.
- 사람 PC (Windows 11 Pro 26200): `ipconfig` 에 IPv6 줄이 없고 `Get-NetIPAddress -AddressFamily IPv6` 에 2 또는 3 으로 시작하는 주소가 없다. 계획 기준대로 **IPv6 없음**으로 기록하고 판정에서 뺀다. 전역 IPv6 UDP 연결은 IPv6 가 있는 회선에서 제품 단계의 연결 시험 때 다시 확인한다.
- 최종 판정: **통과** (Windows 빌드, Linux 와 같은 테스트 결과, `::1` 연결. 전역 IPv6 는 환경이 없어 제외).
- 계획과 다르게 바꾼 점: webrtc-rs 테스트 step 은 `bwe` 가 Linux 에서 이미 실패로 판정되어 `--no-fail-fast` + `continue-on-error` 로 돌렸다.
- 사용량: 이 job 은 12분 41초 (Windows 차감 약 26분). 대부분 cache 없는 Rust 빌드 시간이다.

## T7 WebRTC 라이브러리 결정

- 입력: T4, T5, T6. 규칙(계획 "새로 정한 것" 3번): 질문 5개 통과 수 → Windows 빌드·테스트 → 동점이면 `str0m`.

| 항목 | str0m 0.23.1 | webrtc-rs 0.21.0 |
|---|---|---|
| 1. 외부 후보 | 통과. `Candidate::server_reflexive` 로 직접 추가 (같은 socket 의 host 후보 필요) | 통과. API 없음, SDP 문자열 편집 |
| 2. ICE restart | 통과. 주소 변경 뒤 60ms 재연결 (흉내 네트워크) | 통과. 약 2ms 재연결 (loopback, 주소 변경 없음) |
| 3. H.264 + PLI | 통과. 내장 packetizer, `Event::KeyframeRequest` | 통과. PLI 수신에 직접 만든 interceptor 필요, SPS/PPS 가 MTU 넘으면 조용히 버림 |
| 4. data channel | 통과 | 통과 (받는 쪽 설정 보고가 다름) |
| 5. 대역폭 추정 | **통과**. 1 Mbps 병목에서 0.93 Mbps, 풀리면 4.9 Mbps 까지 회복 | **실패**. 손실 뒤 0.1~0.2 Mbps 로 내려가 회복 못 함 |
| 통과 수 | 5/5 | 4/5 |
| Windows MSVC 빌드·테스트 | 통과, 결과 같음 | 통과, 결과 같음 (bwe 실패도 같음) |
| 구조 | sans-IO: socket·시간을 우리가 넣음. 가짜 네트워크로 결정적 테스트 가능 (spec 9.1 연결 통합·NAT 흉내 테스트) | tokio async, 실제 socket 필요 |
| crypto backend | aws-lc-rs (C 빌드, Windows 에서 됨), rust-crypto·wincrypto 선택 가능 | ring 기본, aws-lc-rs 선택 가능 |

- 선택안: **str0m 0.23.1**. 근거: 규칙의 첫 기준(통과 수 5 대 4)에서 갈린다. 실패한 항목(대역폭 추정)이 spec 6.2 혼잡 대응의 기반이라 제품에 직접 영향이 있다. 추가로 우회 필요 사항이 적고 sans-IO 구조가 spec 9.1 테스트 방식과 맞다.
- str0m 을 쓸 때 제품 계획에 넣을 사항
  - 앱 heartbeat 로 끊김을 판단하고 ICE restart 를 직접 시작한다 (ICE 자체 감지 21초).
  - UPnP 매핑 후보는 SDP 에 srflx 로 알리고, local 에는 같은 socket 의 host 후보를 함께 둔다.
  - 경로 기록은 `set_stats_interval` 의 `PeerStats.selected_candidate_pair` 로 만든다.
  - socket 입출력과 timer 루프는 우리가 만든다 (`ipv6-pair` 의 루프가 최소 형태).
- 상태: **사용자 승인 (str0m)**. 진행 기록 D23, spec 3.3 표와 12절에 반영.

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

- 질문: LocalSystem 이 `CryptProtectData`(entropy 없음, `CRYPTPROTECT_LOCAL_MACHINE` 없음)로 암호화한 데이터를 SYSTEM 은 풀고 관리자 계정은 못 푸나?
- 기준: SYSTEM 복호화 `plaintext=spike-secret`, 관리자 복호화 오류.
- 실행: https://github.com/dev-exintueri/simple-remote/actions/runs/35899681949 (windows-2025, PsExec `-s`, SYSTEM 은 session 0)
- 출력 요약: `RESULT dpapi-system-decrypt ok=true`, 관리자(`runneradmin`, 관리자 권한) 복호화는 `HRESULT(0x8009000B) Key not valid for use in specified state.`, `RESULT dpapi-admin-decrypt ok=true`.
- 판정: **통과**. spec 4절의 "SYSTEM 계정 범위 DPAPI" 가정이 맞다. 관리자 계정도 API 로는 풀 수 없다 (관리자가 SYSTEM 으로 프로그램을 띄우면 풀 수 있다는 점은 DPAPI 의 경계 밖이며, 파일 권한 SYSTEM 전용과 같은 수준의 보호다).
- 실행 중 겪은 문제와 원인 (판정과 무관, 이후 Windows 검사 코드에 적용)
  - PsExec 가 SYSTEM 으로 실행한 프로그램의 stdout 줄을 잃었다 (종료 코드 0 인데 출력 누락). SYSTEM 프로세스가 `cmd /c ... > 파일` 로 쓰고 읽는 방식으로 바꿨다.
  - GitHub pwsh step 은 마지막 외부 프로그램의 `$LASTEXITCODE` 로 끝난다. 기대된 실패(관리자 복호화)가 마지막 명령이면 판정이 통과여도 step 이 실패한다. `exit 0` 을 명시했다.
- Task 11 (사람 PC) 에서도 PsExec 출력이 비면 같은 방식(`cmd /c ... > 파일`)으로 받는다.

## T10 SendInput 절대 좌표

- 질문: physical pixel 좌표를 0..65535 로 바꾸는 공식 A `((x - vx) * 65535) / (vw - 1)` 와 공식 B `((x - vx) * 65536 + vw / 2) / vw` 중 어느 것이 커서를 정확한 픽셀에 놓나?
- 기준: 통과 = 한 공식이 불일치 0건. 두 공식 다 불일치가 있으면 실패. 모니터가 1개거나 배율이 모두 같으면 "부분".
- 실행: 사람 PC (Windows 11 Pro 26200, RTX 3060 Ti, LG UltraFine 1대 3840x2160 배율 150%). exe 는 https://github.com/dev-exintueri/simple-remote/actions/runs/36226592237 (windows-probes, commit `d1a566f`) 의 artifact `win-probes`.
- 출력 요약: `virtual_desktop origin=(0,0) size=(3840x2160)`, `monitor[0] rect=(0,0)-(3840,2160)`, `points=11 formulaA_mismatches=1 formulaB_mismatches=2`.
- 불일치 행 (CSV 원문, 나머지 19행은 `dx=0, dy=0`)

```
target_x,target_y,formula,nx,ny,got_x,got_y,dx,dy
1,1,A,17,30,0,0,-1,-1
1,1,B,17,30,0,0,-1,-1
3838,2158,B,65502,65475,3838,2157,0,-1
```

- 분석: 22행 모두 "Windows 가 픽셀을 `floor(n * vw / 65536)` 로 정한다"는 가정과 맞는다. 예: `n=17` → `17*3840/65536 = 0.996` → 0, `n=65475` → `65475*2160/65536 = 2157.99` → 2157. 이 가정에서 목표 픽셀 x 에 정확히 떨어지는 최소 값은 `ceil((x - vx) * 65536 / vw)` 이다 (공식 C: `((x - vx) * 65536 + vw - 1) / vw`). 공식 A 는 1 근처처럼 작은 좌표에서 모자라고, 공식 B 는 반올림이라 절반 확률로 모자란다. 공식 C 는 이 PC 에서 아직 실행해 보지 않은 추론이다. Microsoft 문서는 변환 규칙을 밝히지 않는다.
- 추가 검사 (계획 밖, 사용자 승인): 공식 C 를 확인하려고 `spikes/sendinput-sweep/sweep.ps1` 을 만들었다. 같은 Win32 API(`SetProcessDpiAwarenessContext` PER_MONITOR_AWARE_V2, `SendInput` ABSOLUTE|VIRTUALDESK, `GetCursorPos`)를 PowerShell P/Invoke 로 불러, 세로 가운데 줄의 x 3840개와 가로 가운데 줄의 y 2160개(6000 지점)에 공식 A, B, C 를 각각 주입하고 되읽는다. 빌드 불필요, 18000번 이동에 6.2초.
  - 1회차: A 222, B 2907, C 77. 세 공식 모두 같은 구간(x 383~1493, 3001~3795, y 12~45)에서 스윕하지 않은 축까지 틀어지거나 20픽셀 넘게 벗어난 행이 있었다. 실행 중 실제 마우스 입력이 섞인 것으로 보고 버렸다.
  - 2회차 (마우스를 들어 둔 상태): `points=6000 formulaA_mismatches=149 formulaB_mismatches=2864 formulaC_mismatches=0`. A 는 x 112건·y 37건, B 는 x 1792건·y 1072건이며 모두 스윕 방향으로 정확히 1픽셀 모자람 (`dx=-1` 또는 `dy=-1`). 위 floor 가정과 맞다.
- 판정: 계획의 두 공식 A, B 는 **실패**. 공식 C 는 6000 지점 불일치 0건으로 기준을 만족하지만 모니터 1대·배율 1종 조건이라 **부분**. 다중 모니터, 음수 원점, 배율 혼합은 확인하지 못했다.
- 설계 영향: spec 6.3 의 좌표 공식은 공식 C `((x - vx) * 65536 + vw - 1) / vw` (y 도 같은 형태)로 바꾼다. 다중 모니터 확인 전까지는 제품 구현에서 주입 후 `GetCursorPos` 되읽기 검사를 테스트로 남긴다.

## T11 MF 하드웨어 인코더 (SYSTEM agent)

- 질문: 사용자 세션에서 도는 SYSTEM 프로세스가 하드웨어 H.264 MFT 를 만들고 D3D11 NV12 texture 를 넣어 인코딩할 수 있나? 안 되면 소프트웨어 MFT 는 되나?
- 기준: 통과 = SYSTEM + 사용자 세션에서 `RESULT mode=hardware ... frames_in=30 frames_out>=25 bytes_out>0`.
- 실행: 사람 PC (Windows 11 Pro 26200, NVIDIA GeForce RTX 3060 Ti driver 32.0.15.9186, GPU 1개). exe 는 https://github.com/dev-exintueri/simple-remote/actions/runs/36226592237 의 artifact `win-probes`. SYSTEM 실행은 관리자 명령 프롬프트에서 `PsExec64.exe -accepteula -s -i 1 cmd /c "C:\spike\mf_probe.exe > C:\spike\mf-system.txt 2>&1"` (Active 세션 ID 1, T9 대로 SYSTEM 쪽 cmd 가 파일로 씀).

| 계정·세션 | 모드 | 인코더 | frames_in | frames_out | bytes_out | 첫 출력 (ms) |
|---|---|---|---|---|---|---|
| lsk30, 1 | hardware | NVIDIA H.264 Encoder MFT | 30 | 30 | 2701 | 4 |
| lsk30, 1 | software | H264 Encoder MFT | 30 | 17 | 14524 | 56 |
| SYSTEM, 1 | hardware | NVIDIA H.264 Encoder MFT | 30 | 30 | 2701 | 2 |
| SYSTEM, 1 | software | H264 Encoder MFT | 30 | 17 | 14524 | 79 |

- 하드웨어 경로의 단계(`MF_TRANSFORM_ASYNC_UNLOCK`, `D3D11CreateDevice(VIDEO|BGRA)`, `MFCreateDXGIDeviceManager`, `SET_D3D_MANAGER`, `SetOutputType`, `SetInputType(NV12)`, `CreateTexture2D(NV12, RENDER_TARGET)`, streaming 시작)는 두 계정 모두 OK. HRESULT 실패 없음.
- 판정: **통과**. spec 6.2 의 "SYSTEM agent 가 사용자 세션에서 하드웨어 인코더 우선" 가정이 이 PC(NVIDIA) 에서 맞다. Intel·AMD GPU 와 hybrid GPU 노트북은 확인하지 못했다.
- 해석할 때 주의할 점
  - 하드웨어 `bytes_out=2701` 은 30 프레임치고 작다. probe 가 texture 를 한 번 만들고 내용을 채우지 않아 매 프레임이 같은 빈 화면이기 때문이다. 인코딩 품질·bitrate 는 이 spike 의 질문이 아니다.
  - 소프트웨어 `frames_out=17` 은 인코더 실패가 아니다. 소프트웨어 경로는 입력 30장 뒤에 `MFT_MESSAGE_COMMAND_DRAIN` 을 보내지 않아 인코더가 들고 있던 뒤쪽 프레임을 꺼내지 않았다 (`mf_probe.rs` `run_software`). 제품 구현은 스트림 끝과 해상도 변경 때 drain 을 해야 한다.
  - `H.264 encoders: 1` 목록에 NVIDIA 가 없는 것은 `MFTEnumEx` 가 `MFT_ENUM_FLAG_HARDWARE` 없이는 하드웨어 MFT 를 돌려주지 않기 때문이다. 하드웨어 선택은 이 flag 로 따로 열거한다.
- 실행 중 겪은 문제: 처음 SYSTEM 실행에서 "액세스 거부"가 났고 `PSEXESVC` 서비스 설치 기록(System 로그 7045)이 없었다. 관리자 권한 창(`whoami /groups` 에서 `High Mandatory Level`)에서 다시 실행해 해결했다. PsExec 출력은 두 번 모두 `cmd exited on DESKTOP-0R38C2A with error code 0.` `IsInRole('Administrators')` 문자열 검사는 이 PC 에서 관리자 창인데도 `False` 를 돌려줘 권한 확인에 쓸 수 없었다.
## T12 egui

- 질문: (1) 1920x1080 영상 texture 를 30fps 로 갱신할 때 한 프레임 처리 시간과 `set`/`set_partial` 차이 (2) Windows 기본 한국어 IME 로 한글 조합·확정·조합 중 백스페이스가 정상인가 (3) 배율이 바뀌면 `pixels_per_point` 가 따라가고 글자가 선명한가
- 실행: 사람 PC (Windows 11 Pro 26200, RTX 3060 Ti, 3840x2160 모니터 1대, Microsoft 한국어 IME 두벌식). exe 는 https://github.com/dev-exintueri/simple-remote/actions/runs/36226592235 (windows-egui, commit `d1a566f`) 의 artifact `win-egui`. 측정값은 에이전트가 창을 화면 캡처해서 읽었다.
- (1) 성능 (배율 150%, 창 약 1290x830 point, "pace video at 30 fps" 켬)

| 업로드 방식 | paints/s | cpu_usage avg / p95 / max (ms) | ui() p95 (ms) | convert p95 (ms) |
|---|---|---|---|---|
| `set_partial` (시작 후 약 20초) | 57 | 3.31 / 6.68 / 9.09 | 5.03 | 3.79 |
| `set_partial` (시작 후 약 30초) | 60 | 2.89 / 5.61 / 8.32 | 3.68 | 2.46 |
| `set_partial` (약 8분 연속) | 58 | 3.07 / 7.17 / 9.06 | 4.64 | 3.35 |
| `set` | 61 | 3.01 / 6.70 / 8.42 | 3.99 | 2.79 |
| `set` (배율 125%, 창 최대화) | 60 | 2.79 / 5.50 / 6.66 | 3.43 | 2.32 |

  - `upload` 는 모든 경우 avg 0.00 ms 다. `set`/`set_partial` 호출은 변경을 기록만 하고 실제 GPU 전송은 render 단계에서 일어나 `cpu_usage` 에 포함된다. 이 PC 에서는 두 방식의 차이가 측정 흔들림보다 작다.
  - `paints/s` 57~61 은 모니터 60Hz vsync 에 맞춘 화면 갱신 수다. 영상 프레임 수(`video frames`)는 30fps pacing 으로 늘었다.
- (2) 한글 IME: 사람 입력 1회 + 자동 입력 1회.
  - 사람 입력: 두 칸 모두 `원격 지원 테스트 한글 입력 확인` 이 그대로 들어갔고 이벤트는 `Preedit("ㄹ") → ("려") → ("력") → Preedit("") + Commit("력")` 흐름.
  - 자동 입력 (`spikes/egui-ime-auto/ime.ps1`, 계획 밖, 사용자 요청): `SendInput` 으로 두벌식 키를 넣어 IME 를 거치게 하고 단계마다 창을 캡처했다. `d n j` 뒤 칸에 조합 중 "워" 가 강조 표시로 보임 (`Preedit("ㅇ") → ("우") → ("워")`). 백스페이스 한 번 뒤 `Preedit("우")` 와 칸에 "우" (자모 하나만 지움). 이어서 `j s` 로 "원". 백스페이스는 IME 가 소비해 egui `Key` 이벤트로 오지 않았다 (egui 가 글자를 한 번 더 지우지 않음). 최종 두 칸 모두 `원격 지원 테스트 한글 입력 확인`.
  - 자동 입력에서 알게 된 것: `ImmGetDefaultIMEWnd` + `IMC_GETCONVERSIONMODE` 로 읽은 모드 값(9)은 실제 한/영 상태와 맞지 않았다 (영문 상태인데 NATIVE 비트가 켜짐). IME 상태 확인은 결과 캡처로 했다.
- (3) DPI: 모니터가 1대라 계획대로 설정에서 배율을 바꿨다. 150% 에서 `pixels_per_point 1.500 native Some(1.5)`, 실행 중 125% 로 바꾸자 `pixels_per_point 1.250 native Some(1.25)` 이고 창의 물리 크기도 1942x1256 에서 1618x1047 로 같은 비율로 줄었다. 캡처에서 글자가 선명했다.
- 판정
  - (1) **통과**: `paints/s` 29 이상, `set_partial` 의 `cpu_usage` p95 5.61~7.17 ms (기준 16 ms 이하).
  - (2) **통과**.
  - (3) **부분**: 배율 변경은 따라가지만, 배율이 다른 두 모니터 사이의 창 이동은 확인하지 못했다.
- 제품 설계에 줄 것: 한글 글꼴은 `C:\Windows\Fonts\malgun.ttf` 를 `FontPriority::Lowest` 로 넣어 동작했다. 입력 칸 테스트 자동화는 `SendInput` 키 입력 + 화면 캡처로 가능하다.
## T13 WiX MSI

- 질문: WiX 7 로 spec 7.1 구성의 MSI 를 만들 수 있나? 설치 후 실제 상태가 선언과 같나? 비정상 종료 뒤 재시작되나? 제거 뒤 흔적이 없나?
- 실행: https://github.com/dev-exintueri/simple-remote/actions/runs/35903301142 (windows-2025, WiX 7.0.0+b8977d6, job 1분 45초)
- 출력 요약: `CHECK` 14개 모두 `ok=true`, `RESULT wix ok=true`.

| 항목 | 결과 |
|---|---|
| 설치 종료 코드 | 0 |
| 서비스 | 존재, `StartName=LocalSystem`, `StartMode=Auto`, `Running` |
| 실패 동작 (`sc qfailure`) | RESTART 5000ms × 3, reset 86400초 |
| 방화벽 규칙 | 프로그램 `C:\Program Files\SimpleRemoteSpike\spike-service.exe` 한정 |
| 데이터 폴더 ACL | `NT AUTHORITY\SYSTEM:(OI)(CI)(F)` 하나, 상속 없음 (SYSTEM 으로 icacls). 관리자 계정은 ACL 읽기와 파일 보기 모두 거부됨 |
| 서비스 로그 기록 | 5초마다 기록 |
| 비정상 종료 뒤 재시작 | 종료 코드 1 로 끝난 뒤 약 7초 만에 새 pid 로 재시작 (pid 4744 → 8848 종료 → 8756) |
| 제거 | 종료 코드 0, 서비스·Program Files·ProgramData·방화벽 규칙 모두 사라짐 |

- 판정: **통과**.
- 실행 중 발견하고 고친 것 (제품 설계에 영향)
  1. `util:RemoveFolderEx` 는 SYSTEM 전용 폴더를 지우지 못한다. immediate custom action 이라 설치 사용자 권한으로 돌아 폴더 목록을 읽다가 `0x80070005` 로 실패하고, WiX 는 이를 성공으로 바꿔 처리해 조용히 폴더가 남는다 (uninstall 로그 확인). `RemoveFiles` 앞에서 LocalSystem 으로 도는 deferred custom action(`Impersonate="no"`, `cmd /c rmdir`)으로 바꿔 해결. 조건 `REMOVE="ALL" AND NOT UPGRADINGPRODUCTCODE` 로 업그레이드 때는 데이터를 남긴다 (업그레이드 경로 자체는 이 spike 에서 실행하지 않음).
  2. 관리자 계정은 SYSTEM 전용 폴더의 ACL 조차 읽지 못한다. 흔적 검사 script(spec 7.3)는 SYSTEM 으로 실행하거나 SYSTEM 으로 읽는 부분을 둬야 한다.
- 설계 영향 (Task 14 에서 spec 반영 승인 요청)
  - spec 7.1: 데이터 삭제와 `SoftwareSASGeneration` 원래 값 복원 둘 다 WiX 선언만으로 안 되고 LocalSystem deferred custom action 이 필요하다. `RegistryValue` 의 Action 은 `append|prepend|write` 뿐이다 (WiX xsd).
  - 이용 조건: WiX 7 은 `-acceptEula wix7` 이 있어야 빌드한다. 개인 비영리는 OSMF 요금 면제.

## 종합

### 판정 표

| task | 질문 | 판정 | 한 줄 요약 |
|---|---|---|---|
| T1 | Actions Windows 빌드 | 통과 | `windows-2025` 에서 빌드·실행, job 약 1~13분 |
| T2 | PAKE | 통과 | `pakery-spake2` 0.6.0 P-256 이 RFC 9382 vector 재현, key 확인 동작. `spake2` 0.4 는 RFC 아님 |
| T3 | 재접속 key 합의 | 통과 | `snow` 0.10 Noise KK 동작. X25519 key 만 받음 |
| T4 | str0m | 통과 5/5 | 외부 후보는 같은 socket 의 host 후보 필요 |
| T5 | webrtc-rs | 4/5 | 손실 뒤 대역폭 추정 회복 못 함 |
| T6 | Windows 빌드·IPv6 | 통과 | MSVC 빌드, `::1` 연결. 사람 PC 는 전역 IPv6 없음 (제외) |
| T7 | WebRTC 라이브러리 | 결정 | `str0m` 0.23.1 (D23, spec 반영 끝) |
| T8 | Workers 로컬 테스트 | 통과 | `@cloudflare/vitest-plugin`, npm 11 필요 |
| T9 | SYSTEM DPAPI | 통과 | 관리자 계정도 API 로 못 풂 |
| T10 | SendInput 좌표 | 공식 A·B 실패, 공식 C 부분 | 올림 공식 C 가 6000 지점 불일치 0. 모니터 1대 조건 |
| T11 | MF 인코더 (SYSTEM) | 통과 | SYSTEM·사용자 세션에서 NVIDIA 하드웨어 MFT, D3D11 texture 30/30 |
| T12 | egui | 통과 2 / 부분 1 | 성능·한글 IME 통과, DPI 는 배율 변경만 확인 |
| T13 | WiX MSI | 통과 14/14 | 데이터 삭제는 LocalSystem deferred custom action 필요 |

### 확인하지 못한 조건 (제품 단계 테스트로 넘김)

- 다중 모니터(배율 혼합, 음수 origin)에서 SendInput 공식 C (T10) 와 egui 창 이동 DPI (T12). 사람 PC 가 모니터 1대라 불가.
- Intel·AMD·hybrid GPU 의 SYSTEM 하드웨어 MFT (T11). 사람 PC 는 NVIDIA 하나.
- 실제 전역 IPv6 UDP 연결 (T6). 사람 PC 회선에 IPv6 없음.

### spec 반영 변경 목록 (승인 요청)

상태: **S1~S9 전부 사용자 승인**. spec 과 진행 기록 결정 기록 D25~D32 에 반영했다 (S7·S8 은 D31 하나, S9 는 D32).

| 번호 | spec 위치 | 지금 | 바꿀 내용 | 근거 |
|---|---|---|---|---|
| S1 | 5.2 3단계, 5.7 둘째 줄 | "SPAKE2, RFC 9382 후보", "`spake2`(RustCrypto) 감사 없음, `opaque-ke` 와 비교" | PAKE 는 `pakery-spake2` `=0.6.0` + `pakery-crypto` P-256-SHA256 (RFC 9382 suite). `crates/auth` 에 격리하고 RFC 9382 vector 를 `crates/auth` 테스트에 둔다. 5.7 한계 문구를 "1인 프로젝트, 미감사, API 변경 잦음 → 버전 고정과 vector 테스트로 완화"로 바꾸고 `opaque-ke` 는 일회용 코드 구조에 맞지 않아 제외했다고 적는다 | T2 |
| S2 | 4절 표, 5.3 재접속 2단계 | 기기 key 는 Ed25519 하나, "Noise KK 후보" | 4절 표에 "재접속 key: 기기마다 X25519 정적 key 쌍, 공개키는 Ed25519 기기 key 로 서명해 신원에 묶음, 보관·폐기는 기기 key 와 같음" 행 추가. 5.3 을 "`snow` Noise KK (`Noise_KK_25519_ChaChaPoly_BLAKE2s`)"로 확정하고, `snow` 에 넘기기 전 key 길이 32 byte 확인을 적는다 | T3 |
| S3 | 5.4 후보 줄 | 후보 종류만 나열 | "공유기 매핑 주소는 SDP 에 srflx 후보로 알리고, local 에는 같은 socket 의 host 후보를 함께 둔다. 경로 기록은 str0m `PeerStats.selected_candidate_pair` 로 만든다" 추가 | T4 |
| S4 | 6.3 셋째 줄 | "반올림 규칙은 문서에 없으므로 되읽어 확인하는 테스트를 둔다" | 변환 공식을 `nx = ((x - vx) * 65536 + vw - 1) / vw` (y 도 같은 형태, 올림)로 정한다. 공식 A·B 는 1픽셀 모자람이 생겨 쓰지 않는다. 되읽기 테스트는 다중 모니터 확인용으로 유지 | T10 |
| S5 | 6.2 코덱 줄 | "하드웨어 MFT 우선, 없거나 실패하면 소프트웨어" | 그대로 두고 두 가지를 덧붙인다: 하드웨어 MFT 는 `MFT_ENUM_FLAG_HARDWARE` 로 따로 열거한다 (없으면 목록에 안 나옴), 스트림 끝·해상도 변경 때 `MFT_MESSAGE_COMMAND_DRAIN` 으로 남은 프레임을 꺼낸다. SYSTEM agent 에서 동작 확인(NVIDIA) 사실을 근거로 남긴다 | T11 |
| S6 | 3.3 UI 줄 | "egui 후보, spike 로 확정" | "egui (eframe 0.36, wgpu)" 로 확정. 한글 글꼴은 시스템 `malgun.ttf` 를 fallback 으로 넣는다. 근거: 1080p30 갱신 cpu p95 약 7ms, 한글 IME 통과 | T12 |
| S7 | 7.1 표 데이터·정책 행, 3.3 설치 줄 | 제거 시 "삭제", "원래 값 복원" | 데이터 폴더 삭제와 `SoftwareSASGeneration` 원래 값 복원은 LocalSystem deferred custom action(`Impersonate="no"`)으로 한다. 업그레이드 때(`UPGRADINGPRODUCTCODE`)는 데이터를 남긴다. 3.3 설치 줄에 "WiX 7.0.0, 빌드에 `-acceptEula wix7`, 개인 비영리는 OSMF 요금 면제" | T13 |
| S8 | 7.3 첫 줄 | PowerShell 검사 스크립트 | 검사 스크립트는 SYSTEM 으로 실행한다 (관리자 계정은 SYSTEM 전용 폴더의 ACL 도 못 읽음) | T13 |
| S9 | 11절 표 | 확인할 spike 목록 | 각 행에 "Phase 0 확인 끝, 결과 문서 T 번호" 를 붙이고, 위 "확인하지 못한 조건" 3가지를 남은 확인으로 적는다 | 전체 |
