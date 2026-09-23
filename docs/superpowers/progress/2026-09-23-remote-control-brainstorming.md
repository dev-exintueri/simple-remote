# 원격 제어 프로그램 brainstorming 진행 기록

이 파일만 읽고 다음 세션이 이어받을 수 있도록 단계가 끝날 때마다 갱신한다.

## 클라우드 세션 인계 (다음 세션이 가장 먼저 읽을 것)

### 현재 상태

- 설계(brainstorming)는 끝났고 SP1 spec 이 사용자 승인을 받았다: `docs/superpowers/specs/2026-09-24-simple-remote-sp1-design.md`.
- 다음 단계는 구현 계획 작성이다. 로컬 세션에서 writing-plans 를 시작하기 직전에 사용자가 이후 작업을 Claude Code 클라우드 세션으로 옮기기로 했다.
- 클라우드 세션에는 이전 대화 맥락, 사용자 전역 설정, 로컬 plugin 이 넘어가지 않는다. 사용자 작업 규칙은 저장소 루트 `CLAUDE.md` 에 옮겨 두었다.

### 다음 할 일: SP1 Phase 0 계획 (spike)

계획 분할 방침: spike 결과가 WebRTC 라이브러리, UI 라이브러리, PAKE 라이브러리를 정한다. 그 전에 전송·UI 코드까지 상세 계획을 쓰면 확인하지 않은 API 를 추측하게 되므로, SP1 을 순차 계획 여러 개로 나눈다. 먼저 Phase 0(spec 11절 spike) 계획을 상세히 쓰고, 이후 계획은 spike 결과가 나온 뒤 하나씩 쓴다. 계획 파일 위치는 `docs/superpowers/plans/`.

spec 11절 spike 를 실행 위치로 나누면 다음과 같다.

| spike | 실행 위치 | 이유 |
|---|---|---|
| WebRTC 라이브러리 (`str0m` / `webrtc-rs`): 외부 후보 추가, ICE restart, H.264 RTP, data channel, 대역폭 추정 | 클라우드(Linux) | loopback 과 network namespace 로 확인 가능. Windows IPv6 후보 동작만 Windows 필요 |
| PAKE (`spake2` / `opaque-ke`, RFC 9382 테스트 값) | 클라우드 | 순수 Rust |
| 재접속 key 합의 (Noise KK crate) | 클라우드 | 순수 Rust |
| Cloudflare Workers 로컬 테스트 도구 (Durable Object, WebSocket) | 클라우드 | Node 로컬 실행, Cloudflare 계정 불필요 |
| WiX Toolset 버전·이용 조건 | 클라우드(문서 조사) + Windows(MSI 빌드) | |
| egui: 영상 텍스처 갱신 지연, 한글 IME, Per-Monitor v2 DPI | Windows (사람 실행) | IME, DPI 는 Windows 에서만 의미 있음 |
| SYSTEM 계정 DPAPI | Windows (사람 실행) | |
| MF 하드웨어 인코더를 SYSTEM agent 에서 생성, D3D11 texture 입력 | Windows (사람 실행, GPU 필요) | |
| `SendInput` 절대 좌표 반올림 되읽기 | Windows (사람 실행) | |

Windows spike 는 클라우드 세션이 검사 코드와 실행·결과 보고 방법을 준비하고, 사람이 Windows PC 에서 실행해 결과를 돌려준다.

### 보류 중인 사람 판단

- Windows spike 를 어느 PC 에서 돌릴지. 개발에 쓰던 PC 는 회사 관리 PC(Windows 11 Enterprise)로 보여서, SYSTEM 서비스 설치나 입력 주입 테스트는 개인 Windows PC 에서 하는 것을 권했다. 사용자는 아직 답하지 않았다.

### 환경 메모

- 개발에 쓰던 로컬 WSL 의 Rust 는 1.88 이라 업데이트가 필요했다. 클라우드 세션에서는 `rustc --version` 으로 버전을 먼저 확인하고, 필요하면 `rustup update stable` 을 쓴다.
- Windows 쪽 빌드 도구(MSVC linker 등)는 준비되어 있지 않다. Windows spike 를 어떻게 빌드할지(Windows PC 에서 직접 빌드 / Linux 에서 교차 빌드)는 Phase 0 계획에서 정한다.

### 클라우드 세션 첫 메시지 (사용자가 붙여 넣을 문장)

```
CLAUDE.md 와 docs/superpowers/progress/2026-09-23-remote-control-brainstorming.md 의 "클라우드 세션 인계" 절을 먼저 읽어 줘.
승인된 spec(docs/superpowers/specs/2026-09-24-simple-remote-sp1-design.md) 기준으로 SP1 Phase 0(spec 11절 spike) 구현 계획을 작성해 줘.
superpowers plugin 이 있으면 writing-plans skill 을 쓰고, 계획을 쓰기 전에 필요한 라이브러리 API 는 문서나 소스로 확인해 줘.
계획이 끝나면 나에게 검토를 요청하고, 승인 전에는 구현을 시작하지 마.
```

## 요청 요약 (사용자 원문 기준)

- P2P 연결 기반 원격 제어 프로그램. 연결 중개 서버는 없으면 좋지만, 불가피하면 연결 초기화를 돕는 서버는 허용.
- Windows 우선, 추후 macOS 지원.
- 기본 보안 처리가 기반이어야 함.
- 해상도 관련 이슈 전부 처리.
- 멀티 모니터는 가능할수록 좋음 (후속 기능).
- 제어권 관리, 단축키 지원.
- 클립보드 공유.
- 원격 프로그램이 일반적으로 제공하는 기능 전부 지원.
- TeamViewer 기능을 조사해 도입 여부 검토.

## 경로 분류

architectural. 새 프로젝트이고 네트워크·보안·화면 캡처·입력 제어 등 독립 하위 시스템이 여러 개라 sub-project 로 나눠 spec 을 따로 쓴다.

## 단계 상태

| 단계 | 상태 | 비고 |
|---|---|---|
| 1. 프로젝트 맥락 확인 | 완료 | 빈 repo, 커밋 없음, branch `master` |
| 2. 기술·기능 조사 | 완료 | 3건 모두 `docs/research/` 에 있음 |
| 3. 의도·제약 확인 질문 | 완료 | D1~D12 |
| 4. 접근 방식 2~3개 제시 | 완료 | D13: Rust + WebRTC |
| 5. 섹션별 설계 승인 | 완료 | 섹션 1~6 승인 (D14~D20) |
| 6. spec 작성·커밋 | 완료 | `docs/superpowers/specs/2026-09-24-simple-remote-sp1-design.md`, commit `4f6f9fe` (develop 브랜치, 조사 문서 3건 포함) |
| 7. spec self-review | 완료 | 수정: 코드 형식(숫자 6자리), 전송 해상도 비율 유지, ICE restart 20초 기준점, 대기 시간 유지·초기화 조건, 클립보드 제외 형식 단순화 |
| 8. 사용자 spec 검토 | 완료 | 사용자 승인 (수정 없음). 지시: commit 후 `https://github.com/dev-exintueri/simple-remote.git` 에 push |
| 9. writing-plans 호출 | 진행 중 | 진행 기록 commit `b99c675`, `develop` 을 origin(`dev-exintueri/simple-remote`, private)에 push 완료. 원격 기본 브랜치 develop |

## 조사 산출물 (작성 중)

- `docs/research/2026-09-23-teamviewer-features.md`: TeamViewer 기능 목록과 분류
- `docs/research/2026-09-23-p2p-networking-and-security.md`: P2P 연결, NAT traversal, 보안, 오픈소스 참고 구현
- `docs/research/2026-09-23-host-platform-windows-macos.md`: 화면 캡처, 인코딩, 입력, DPI, 멀티 모니터, 클립보드, 권한

## 조사에서 확인된 핵심 사실 (설계 제약)

host 플랫폼 조사(`docs/research/2026-09-23-host-platform-windows-macos.md` 11절) 요약:

- UAC 창, 잠금·로그인 화면을 보고 조작하려면 대상 세션 안의 LocalSystem 프로세스가 필요. 구조는 "LocalSystem 서비스 + 세션별 SYSTEM helper" 2단. 서비스 없이 도는 모드(Quick Assist 포함)는 host 앞 사람이 UAC 창을 직접 눌러야 함. RustDesk portable 모드는 UAC 승인 1회 후 SYSTEM 프로세스를 띄움.
- 캡처·입력·클립보드는 사용자 세션 helper 가 맡고, 서비스와는 ACL 을 건 IPC 로 통신. desktop 전환(UAC, 잠금, 해상도 변경) 때마다 캡처·입력을 다시 붙이는 상태 기계 필요.
- 기본 캡처는 DDA(Desktop Duplication). WGC 는 SYSTEM 모드에서 안 되고, 테두리 끄기는 build 20348+ 만 가능. hybrid 노트북 외장 GPU 는 DDA 불가라 GDI fallback 필요.
- Ctrl+Alt+Del 은 viewer 에서 가로챌 수 없음. host 에서 SendSAS(서비스 + 정책 설정)로만 보냄. viewer 에 "특수 키 보내기" 메뉴 필수.
- SendInput 이 UIPI 에 막혀도 실패를 알 수 없음. 관리자 창 조작은 SYSTEM helper 에서만.
- host 는 Per-Monitor v2 DPI aware, 좌표는 physical pixel virtual desktop(음수 포함)으로 통일.
- 키 입력은 scancode + 문자 둘 다 전송. 한/영 키는 host 키보드 종류에 따라 VK_RMENU 또는 VK_HANGUL.
- 글자 선명도(YUV 4:4:4)는 Windows 내장 decoder 미지원. 코덱·chroma 는 연결마다 협상하는 구조 필요. x264 GPL, OpenH264 는 Cisco binary 조건, VP9/AV1 은 BSD. RustDesk(AGPL-3.0), Sunshine(GPL-3.0)은 참고만.
- 원격 파일 붙여넣기는 OleSetClipboard + FILEDESCRIPTOR/FILECONTENTS delayed rendering 필요.
- 배포: 서명해도 초기엔 SmartScreen 경고. EV 즉시 통과는 2024년에 없어짐. 원격 제어 상태를 숨기면 Defender unwanted software 기준 위반.

P2P 조사(`docs/research/2026-09-23-p2p-networking-and-security.md` 1, 3.7, 4, 10, 11장) 요약:

- relay 없이 hole punching 만: 연결의 약 10~30% 실패 (추정). TeamViewer 직접 연결 70%, libp2p 측정 70% ± 7.1% (조건부).
- 휴대폰 망: cellular AS 의 90% 이상이 CGN, 그중 약 40% 가 symmetric → 휴대폰 hotspot 쪽은 hole punching 실패가 잦음. 양쪽 symmetric 이면 relay 없이는 불가.
- UPnP 응답 가정 약 35% (2011 자료). 한국 IPv6 사용률 17.42%.
- viewer reverse 연결을 더하면 viewer 가 집에 있을 때 실패가 거의 0. 사용자는 주로 집 밖(D6)이라 효과 제한.
- signaling: Cloudflare Workers + Durable Objects 무료 plan 으로 충분. STUN: `stun.cloudflare.com` "free and unlimited".
- Oracle Cloud Always Free: outbound 월 10 TB 무료, E2.1.Micro 는 대역 최대 50 Mbps, idle 회수 정책 있음. → D5 의 이유(트래픽 비용)가 이 조건에서는 성립하지 않을 수 있음. 사용자에게 재확인 필요.
- 선택지 3개 (공통 전제: 경로 경주 + PAKE 로 채널 묶기): 1) WebRTC ICE (TURN 없음) + Workers signaling 2) QUIC(iroh relay 끔 / quinn): iroh 는 relay 끄면 hole punching 도 사라짐 3) RustDesk fork (hbbs 만): AGPL-3.0, 암호화 없이 진행하는 경로와 비 PAKE 비밀번호 수정 필요, reverse 모드 없음, hbbs 용 VM 필요.

## 결정 기록

- D1. 최우선 사용 상황: 가족·지인 원격 지원 (attended access). 사용자 본인이 viewer, 컴퓨터를 잘 모르는 상대가 host.
  - 함의: host 는 설치 없이 실행 가능할수록 좋음, 접속 ID + 일회용 코드 표시, 접속 허락 창, 연결 중 표시와 즉시 끊기 필요.
  - 함의: 상대 네트워크를 원격으로 고쳐 줄 수 없으므로 직접 연결 실패 시 대비책(relay)의 중요도가 올라감 (조사 결과로 근거 확인 예정).
  - 함의: 설치 지원 중 뜨는 UAC 창 조작 가능 여부가 핵심 제약 후보 (조사 결과로 확인 예정).
  - unattended access 는 후순위 (예: 부모님 PC 정기 관리용으로 나중에 추가).
- D2. 구현 언어 기준: 문제에 가장 적합한 언어. 사용자가 모르는 언어여도 됨. 조사 결과(라이브러리 성숙도, OS API 접근성, 참고 구현)를 근거로 추천한다.
- D3. 연결 요구사항 (사용자 추가 발언): "연결에 많은 노력을 들이지 않고, host 가 외국이나 공유기 환경이어도 쉽게 접속 가능해야 함."
  - 해석: 연결 성공률 최우선. 포트포워딩 등 host 쪽 네트워크 설정 요구 금지.
  - 함의: hole punching 실패 대비 relay 경로가 사실상 필요할 가능성 큼 (조사 결과로 근거 확인 예정). UDP 차단 망 대비 TCP 443 우회 경로 검토. 외국 host 는 relay 위치가 지연을 좌우.
  - 처음 요청의 "가능하면 중개 서버 없이"는 "연결 시작 서버 + 직접 연결 실패 시에만 쓰는 relay"로 바뀔 가능성이 큼. 조사 결과 도착 후 사용자에게 근거와 함께 재확인한다.
- D4. 서버 운영 (사용자 답변): 저렴한 클라우드 무료 tier 운영 가능. 필요하면 Cloudflare 를 앞에 붙여 hostname 사용 가능.
  - 사용자 의도 명확화: 중개 서버는 "첫 연결을 쉽게 하는 용도"로만 둔다. 세션 트래픽 전체를 전달하는 relay 는 원하지 않음. 서버가 필요 없으면 없는 게 가장 좋음.
  - 미결 쟁점: D3(외국·공유기 환경에서도 쉬운 연결)과 "data relay 없음"이 부딪힐 수 있음. relay 없이는 직접 연결이 안 되는 네트워크 조합에서 접속 자체가 실패함.
  - 조치: P2P 조사 agent 에 추가 범위 요청. (1) relay 없이 직접 연결 성공률을 높이는 기법(UPnP/NAT-PMP/PCP 자동 포트 매핑, IPv6, symmetric NAT 대상 port prediction, TCP simultaneous open) (2) 직접 연결 실패 비율 실측 자료 (3) 스택별 relay 끄기 가능 여부 (4) Cloudflare/무료 tier 에서 signaling·STUN 운영 가능성 (5) "data relay 없음" 설계의 예상 실패율 표.
  - 다음 결정: 조사 수치를 근거로 사용자에게 "relay 없음 / 최후 수단으로만 relay / 기타"를 다시 묻는다.
- D5. relay 는 처음부터 설계에서 제외 (사용자 결정, 이유: 트래픽 비용 감당 불가). D4 의 미결 쟁점 중 "relay 여부"는 이것으로 닫힘.
  - 사용자 질문: "처음부터 직접 연결이 무조건 가능한 구조로 설계할 수 있나?"
  - 답: 양쪽 네트워크가 임의라면 "무조건"은 불가능 (양쪽 모두 inbound 불가 + symmetric NAT 또는 UDP 차단 조합은 제3자 전달 없이는 경로가 없음).
  - 대신 제안: viewer 가 사용자 본인이라는 점을 이용한 역방향 연결(viewer 가 listen, host 가 outbound 접속). 사용자 쪽 네트워크만 열면 host 는 공유기·외국 환경에서도 대부분 연결됨. Cloudflare DNS hostname 을 쓰면 연결 시작 서버도 없앨 수 있는 가능성.
  - 제안 구조: 가능한 경로(역방향, 정방향 + 자동 포트 매핑, hole punching, IPv6)를 동시에 시도하고 먼저 성공한 경로 사용. 전부 실패 시 원인과 해결 방법 표시.
  - 조치: P2P 조사 agent 에 역방향 연결 선례, viewer 쪽 열기 방법, Cloudflare DNS/proxy 제약 조사 추가 요청.
  - 다음 질문: 사용자가 주로 어디서 접속하는지, 자기 쪽 네트워크를 열 수 있는지.
- D6. 사용자(viewer) 접속 위치: 주로 집 밖 (노트북, 카페·회사·핫스팟 등).
  - 함의: 역방향 연결은 보조 경로로 격하 (집에 있을 때만 유효).
  - 핵심 경로: host(가족 집 공유기) 쪽 자동 포트 매핑(UPnP/NAT-PMP/PCP) 후 viewer 가 직접 접속 + UDP hole punching + IPv6 직접. 모두 동시 시도.
  - 남는 실패 조합: host 공유기 자동 포트 매핑 불가 + viewer 쪽이 hole punching 에 불리한 망(UDP 차단 회사망, 통신사 NAT 뒤 핫스팟 등). 대응은 원인 표시와 "viewer 쪽 네트워크 바꾸기" 안내. 실패 비율은 조사 수치로 사용자에게 다시 보여 준다.
  - 다음 질문: viewer 노트북 OS (MacBook 이면 viewer 는 첫 버전부터 macOS 지원 필요).
- D7. 플랫폼 범위: 사용자는 Windows 노트북과 MacBook 을 둘 다 씀. 첫 버전은 host·viewer 모두 Windows 만. macOS 는 나중에 별도 sub-project 로 진행.
  - 설계 제약: macOS 를 나중에 붙일 때 전면 재작성이 없도록 platform 의존 부분(캡처, 입력, 권한, UI)을 경계 뒤로 분리하고, 언어·UI 선택도 macOS 로 확장 가능한 쪽을 우선한다.
  - 다음 질문: 가족 PC 의 Windows 버전 범위 (캡처 API 선택에 영향).
- D8. host Windows 버전: Windows 11 위주, Windows 10 도 호환 지원.
  - 사용자 요구: Windows 10 에서 안 되는 기능은 명확히 구분해 API 를 호출하고, 그 사실을 viewer(도와주는 쪽)에 표시한다.
  - 설계 함의: host 가 실행 시 OS 버전과 사용 가능한 기능(capability)을 감지해 연결 시 viewer 에 알리고, viewer UI 는 쓸 수 없는 기능을 이유와 함께 표시한다. 기능별 API 호출은 capability 확인 뒤에만 한다.
  - 다음 단계: 조사 결과 3건 도착 대기. 도착 후 (1) relay 없는 설계의 실패 비율 수치 공유 (2) 기존 오픈소스(RustDesk 등) 활용 vs 새로 만들기 질문 (3) 첫 버전 범위 제안 (4) 언어·구조 추천 2~3안.
- D9. host 형태: 처음부터 설치형 서비스 (사용자 선택). 조건: 제거 시 깔끔하게 제거되어야 함.
  - 근거: UAC 창·Ctrl+Alt+Del·잠금 화면 조작은 LocalSystem 서비스 + 세션별 SYSTEM helper 구조에서만 가능 (host 플랫폼 조사 11절 1, 2, 6, 7항).
  - 함의: 가족은 처음 한 번 설치하고 UAC 를 승인. 나중 unattended access 의 기반도 여기서 나옴.
  - 제거 시 원상 복구 대상(설계에서 확정): 서비스 등록, 프로그램 파일, 설정·key, 방화벽 규칙, 공유기에 연 포트(UPnP 매핑), SoftwareSASGeneration 정책, 시작 메뉴 항목. 제거 후 흔적 검사 테스트를 둔다.
  - 설계 제안(섹션 설계에서 확인받을 것): 첫 버전은 attended 만이므로 서비스는 상시 대기만 하고, 가족이 host 창을 열어 코드가 떠 있는 동안에만 연결을 받는다.
- TeamViewer 조사 도착 (`docs/research/2026-09-23-teamviewer-features.md`, 155개 기능, A 26 / B 37 / C 37 / D 55).
  - D5 와 충돌하는 사실: TeamViewer 는 직접 연결이 약 70%, 나머지는 relay 경유 (TeamViewer security statement). P2P 조사 수치와 함께 사용자에게 재확인 예정.
  - 보안 기준선: mutual TLS 1.3 + PFS + AES-256-GCM, SRP v6 password, 실패 시 지수 대기(24회에 17시간), attended 동의 prompt, stealth mode 없음(세션 표시), incoming 로그.
  - 2026년 TeamViewer 취약점은 동의·승인 단계 우회 → 동의 확인은 host 에서 모든 경로에 동일하게 강제.
  - 다음 질문: 사용자가 말한 "제어권 관리"의 의미 (P2P 조사와 독립).
- D10. 제어권 관리 범위 (사용자 답변): "가족이 즉시 되찾기", "가족 입력 막기·화면 가리기" 필요.
  - 사용자 메모 원문: "'설치' 자체가 이미 허용했다는 의미. 대부분 기본 허용. '원격 접속 제어중'을 화면에 명확히 표시하는 것으로 대체".
  - 빠지는 것: 보기 전용 ↔ 제어 요청 흐름, 기능별 허용 권한(Custom). 연결되면 기능은 기본 허용.
  - 대체 장치: host 화면에 "원격 제어 중"을 명확히 표시.
  - 미결: "설치 = 허용"이 연결할 때의 허락(attended 동의)까지 없애는 뜻인지 확인 필요. 그렇다면 사실상 unattended access 이고 D1(unattended 후순위), D9 제안(host 창을 열었을 때만 연결 받기)이 바뀜.
  - 그다음 질문: relay 재확인 (Oracle Always Free outbound 10 TB 로 비용 이유가 약해짐, 사용자가 주로 집 밖이라 실패율 10~30% 추정).
- D11. 접속 허락 규칙 (사용자 답변): 연결은 매번 가족 확인. 연결이 끊긴 뒤 1시간 이내 재접속은 확인 없이 허용하며, host 재부팅 후 재연결도 자동 허용.
  - 구현 방향: 첫 허락 시 host 가 세션 전용 재접속 허가증을 발급하고, viewer 기기 key 에 묶어 1시간 뒤 폐기. 재부팅을 견디도록 양쪽 모두 암호화해 디스크에 보관 (host 는 DPAPI machine scope 후보). 서비스는 부팅 직후 로그인 화면 단계부터 재접속을 받음 (reboot-and-reconnect).
  - 서비스 연결 수용 규칙 (D9 제안 수정): 새 연결은 가족이 host 창을 열고 코드가 떠 있는 동안에만 받음. 재접속은 유효한 허가증이 있는 동안 host 창이 닫혀 있어도 받음 (그동안 signaling 에 계속 등록).
  - 제안 (사용자에게 알림, 이의 없으면 확정): 가족이 끊기 버튼이나 비상 단축키로 직접 끊은 경우 허가증 즉시 폐기. 네트워크 끊김, 재부팅, viewer 쪽 종료만 자동 재접속 대상.
- D12. relay 재확인 결과: 첫 버전은 relay 없이 시작하고 실측 후 판단 (사용자 선택, 추천안). D5 를 대체.
  - 첫 버전에 넣을 것: 연결마다 경로별 시도 결과(성공/실패, 실패 원인 범주)를 기록. 실패가 잦으면 Oracle Always Free relay 를 경로 하나로 추가 검토.
  - 구조 조건: 경로 경주 구조에서 relay 는 후보 경로 하나로 붙일 수 있어야 함 (재작성 없이).
  - 사용자 수용 사항: 그동안 집 밖 접속에서 약 10~30% 실패를 감수.
- 다음 단계: 접근 방식(언어 + 전송 stack) 2~3안 제시. 추천: Rust + WebRTC(ICE, TURN 없음) 새로 만들기.
- D13. 접근 방식: A. Rust + WebRTC 로 새로 만들기 (사용자 선택, 추천안). D2 의 언어 추천(Rust)도 이것으로 확정.
  - 근거: SYSTEM 서비스가 인터넷 입력을 처리하므로 메모리 안전성 필요, `windows-rs` 로 DDA/Media Foundation/서비스 API 접근, 런타임 없는 단일 실행 파일, 경로 경주(ICE)와 영상 전송(RTP)을 표준에 맡김, Cloudflare Workers 무료 signaling 과 맞음, 라이선스 제약 없음.
  - 뺀 대안: B(QUIC, hole punching·혼잡 제어 직접 구현 부담), C(RustDesk fork, 암호화 없이 진행하는 경로 등 보안 핵심 결함, AGPL-3.0, hbbs VM 상시 운영).
  - 미정: Rust WebRTC 라이브러리(str0m / webrtc-rs)는 구현 계획 단계 spike 로 확정.
- 다음 단계: 섹션별 설계 제시. 첫 섹션은 sub-project 분해와 첫 버전(SP1) 범위.
- D14. 섹션 1 승인: sub-project 분해와 SP1 범위 (사용자 승인, 수정 없음).
  - SP1 = Windows 가족 지원 최소 완성품. SP2 제어권 확장(입력 막기, 화면 가리기), SP3 클립보드·파일, SP4 멀티 모니터·해상도 변경, SP5 연결 성공률 확장, SP6 macOS viewer, SP7 macOS host, 이후 TeamViewer B/C 기능 검토.
  - SP1 포함: 설치·깨끗한 제거(흔적 검사), viewer 는 설치 없는 단일 실행 파일, 접속 ID + Cloudflare signaling/STUN + 경로 경주(직접, hole punching, IPv6, host UPnP/PCP) + 경로별 기록 + 실패 원인 안내, PAKE 코드 + 채널 묶기 + 허락 창 + 1시간 재접속 허가증 + 시도 제한 + 접속 기록 + "원격 제어 중" 표시, UAC·잠금·로그인 화면 조작, 모니터 1개씩 보기 + 전환, HW 인코딩 우선 + SW 대체, 커서, DPI·화면 맞춤/원래 크기·해상도 변경 추종, 마우스·키보드(한/영·한자), 단축키 보내기 토글, 특수 키 메뉴, 가족 끊기 버튼·비상 단축키, 텍스트 클립보드(viewer 창 포커스 시만), Win10/11 기능 감지 표시, 원격 재부팅 후 자동 재접속.
  - SP1 제외: relay, 다자 접속, 무확인 무인 접속, 모바일, TLS 검사 proxy 회사망 host, 코드 서명(설치 시 SmartScreen 경고는 "추가 정보 → 실행" 안내).
- 다음 단계: 섹션 2 전체 구조(프로세스·모듈·서버).
- D15. 섹션 2 승인: 전체 구조 (사용자 승인, 수정 없음).
  - host 3 프로세스: service(SYSTEM, session 0, agent 감독 + SendSAS 대행, 인터넷 통신 없음) / agent(SYSTEM, 활성 콘솔 세션, 네트워크·보안·캡처·인코딩·입력·클립보드·기록) / host UI(가족 권한, ID·코드·허락 창·제어 중 표시·끊기). agent ↔ host UI 는 ACL 건 named pipe.
  - 네트워크는 agent 에 둠. 세션 전환(로그아웃·사용자 전환)·재부팅 시 연결이 끊기고 viewer 가 재접속 허가증으로 자동 재접속.
  - viewer: 단일 프로세스, 설치 없음, 기기 key·허가증은 DPAPI(사용자 범위).
  - signaling: Cloudflare Worker(TypeScript) + host 별 Durable Object. 서버는 IP·연결 대상만 보고 화면·입력·코드는 못 봄.
  - Rust workspace: protocol / auth / transport / codec / platform-win + 실행 파일 4개(host-service, host-agent, host-ui, viewer) + signaling/ + installer/(WiX MSI).
  - UI 라이브러리 후보 egui, 구현 계획 spike 로 확정 (영상 그리기 성능, 한글 입력).
- 다음 단계: 섹션 3 연결·보안 흐름.
- 사용자 질문 (섹션 3 제시 전): "도움을 주는 쪽은 mobile app 도 추가할 수 있을까?" → 실행 지시가 아니라 검토 요청으로 처리.
  - 평가: 가능. viewer 는 캡처·권한이 없는 가벼운 쪽. 영향: 휴대폰 망(CGN, symmetric 약 40%)에서 relay 없이 실패율 증가, 터치 → 마우스 변환·화면 확대·특수 키 도구 막대 필요, iOS 백그라운드 시 연결 중단(재접속 허가증으로 보완), iOS 배포는 유료 개발자 계정 필요.
  - 지금 바꿔야 나중에 재작성 없음: (1) viewer 를 `viewer-core`(Rust 라이브러리: 연결·보안·디코딩·입력/클립보드 프로토콜)와 데스크톱 UI 로 분리 (2) 코덱 협상에서 H.264 를 항상 가능한 기본값으로 유지 (모바일 하드웨어 디코딩 공통).
  - 사용자에게 추가 여부와 우선순위를 질문 중. 섹션 3 은 답을 받은 뒤 제시.
- D16. 모바일 viewer: 나중 하위 프로젝트 + 지금 구조 대비 (사용자 선택, 추천안). 순서 메모 없음 → SP5 바로 다음(SP6). macOS viewer 는 SP7, macOS host 는 SP8 로 이동.
  - 섹션 2 수정: viewer 를 `viewer-core`(Rust 라이브러리) + 데스크톱 UI 실행 파일로 분리.
  - 섹션 4 반영 예정: H.264 를 항상 가능한 기본 코덱으로 유지.
  - 모바일 UI 도구(각 OS 전용 / Flutter 등)는 해당 하위 프로젝트 시작 시 결정.
- 다음 단계: 섹션 3 연결·보안 흐름 제시.
- D17. 섹션 3 승인: 연결과 보안 흐름 (사용자 승인, 수정 없음).
  - 신원: host 기기 key(Ed25519, SYSTEM 계정 DPAPI + SYSTEM 전용 파일 권한), 9자리 접속 ID(서버 발급, host 공개키에 묶음, 신뢰 근거 아님), viewer 기기 key(사용자 DPAPI) + 기기 이름.
  - 첫 연결: host 창 열림 시에만 "새 연결 받기" 등록 → 6자리 일회용 코드 → signaling 경유 PAKE 먼저 → PAKE key 로 SDP(후보·fingerprint) 암호화·인증 교환 → ICE 경주 + DTLS → 채널 안에서 기기 공개키 교환 → 허락 창(기기 이름, 처음/이전 기기 표시, 30초 무응답 거절) → 세션 시작, 코드 교체.
  - 재접속: 허가증(번호, viewer 공개키, 끊긴 시각 + 1시간) viewer key 서명 필수. 저장된 공개키로 상호 인증 key 합의(Noise KK 후보) → 같은 SDP 교환 → 허락 창 없이 시작 + "재접속됨" 알림. 네트워크 끊김·재부팅은 viewer 자동 재시도(간격 증가, 1시간까지), viewer 가 끊은 경우 "최근 연결"에서 수동. 유효 허가증 있으면 host 창 닫힘·로그인 화면에서도 "재접속만 받기" 등록. 폐기: 가족 끊기/비상 단축키, 1시간 경과, "재접속 허가 모두 취소", 제거.
  - 공격 대비: 실패마다 대기 2배, 5회 실패 시 코드 교체, signaling IP·ID 별 요청 제한, 등록 시간 최소화(UPnP 매핑도 그때만).
  - 기록: host 접속 기록(host 창에서 열람), viewer 경로 기록, 실패 원인 범주별 안내.
  - 한계: 로그인 화면에서는 "원격 제어 중" 표시 불가(로그인 직후부터), Rust PAKE 라이브러리 미감사(RFC 9382 테스트 값 검증, auth 격리, opaque-ke 비교), SYSTEM DPAPI 동작 확인 필요.
- 다음 단계: 섹션 4 화면·입력·클립보드.
- D18. 섹션 4 승인: 화면·입력·클립보드 (사용자 승인, 수정 없음).
  - 캡처: 모니터별 DDA 기본, GDI 대체, WGC 미사용. desktop 전환·해상도 변경 등은 전용 thread 상태 기계로 재부착.
  - 코덱: H.264 만, Media Foundation(HW MFT 우선, 내장 SW MFT 대체) 인코딩·디코딩. 코덱 협상 구조는 처음부터. GPU 에서 BGRA→NV12 + 크기 조정, 기본 최대 1920x1080 + "원본 화질" 옵션. 저지연 모드, B-frame 없음, viewer 가 key frame 요청, 혼잡 시 bitrate → FPS 순으로 낮춤. 커서는 모양·위치 별도 전송(영상 합성 방식 자리도 둠).
  - 한계: 4:2:0 이라 색 글자 약간 번짐(VP9/AV1 4:4:4 는 나중), HDR 처리 없음, Windows N/KN 에디션은 영상 불가(연결 시 이유 표시).
  - DPI: host PMv2, 실제 픽셀 virtual desktop 좌표(음수 포함), viewer 는 보고 있는 모니터 기준 픽셀 좌표 전송, 0..65535 변환은 주입 후 커서 위치 되읽기 테스트. viewer 는 화면 맞춤/원래 크기, DPI 인식. 해상도·회전·DPI 변경 자동 추종. 원격 해상도 변경은 SP4.
  - 입력: 마우스 절대 위치·버튼 5개·휠 원래 이동량(세로·가로). 키보드는 scancode + 문자 둘 다 전송, SP1 은 scancode 방식. 한/영·한자는 host 키보드 종류에 맞춰 변환. 단축키 보내기 토글(전용 thread LL hook). 특수 키 메뉴: Ctrl+Alt+Del(SendSAS), Win+L, Ctrl+Shift+Esc, Alt+Tab, Win, Print Screen. viewer 예약 조합: Ctrl+Alt+Enter(전체 화면), Ctrl+Alt+Home(단축키 보내기 토글).
  - 제어권 표시: 가족 화면 상단 항상 위 막대 "[기기 이름]이 원격 제어 중 [끊기]". 비상 단축키 Ctrl+Alt+F8, 실제 키보드 입력만 인정, agent 는 이 조합 주입 금지.
  - 클립보드: 텍스트 양방향, viewer 원격 창 포커스 시만, 포커스 얻을 때 viewer → host 전송, 자기 되돌림 방지 표식, 클립보드 기록·클라우드 클립보드 제외 표시, 1 MiB 상한.
  - 지원 OS: Windows 11 23H2 이상, Windows 10 22H2. 세션 시작 시 capability 전송(OS build, HW 인코더, 모니터별 캡처 방식, 미디어 기능 유무).
  - 원격 재부팅: service 가 재부팅, 부팅 후 로그인 화면 세션 agent, viewer 허가증 자동 재접속. 안전 모드 재부팅 제외.
- 다음 단계: 섹션 5 설치·제거·운영.
- D19. 섹션 5 승인: 설치·제거·운영 (사용자 승인, 수정 없음).
  - host 설치: WiX MSI(버전·이용 조건은 구현 계획에서 확인). 실행 파일 3개(Program Files), 서비스(자동 시작, SYSTEM, 실패 시 재시작), 데이터(ProgramData, SYSTEM 전용, 업그레이드 시 유지·제거 시 삭제), 방화벽 규칙(agent 한정), SoftwareSASGeneration(원래 값 기록 → 제거 시 복원), 바로가기 "원격 지원 받기", UPnP 매핑은 대기 중에만 + 유효 시간(예: 1시간, 갱신).
  - host 창: 자동 실행·트레이 없음. 세션 중 창이 닫혀 있으면 service 가 가족 권한으로 띄워 "원격 제어 중" 표시.
  - 첫 설치: 링크 → 실행 → SmartScreen "추가 정보 → 실행" → UAC "예" → 바탕화면 바로가기. 전화 안내 전제.
  - viewer: 단일 exe, 데이터 %APPDATA%\SimpleRemote, "내 데이터 모두 삭제" 메뉴.
  - 제거 흔적 검사: 깨끗한 VM 에서 설치 전/제거 후 상태 비교 PowerShell 스크립트(신규 작성 예정), 차이 1건이라도 실패, 출시마다 실행.
  - 업데이트: SP1 자동 업데이트 없음. 원격 제어로 가족 PC 브라우저에서 새 MSI 실행(UAC 도 원격 조작). 업그레이드 시 연결 끊김 → 허가증 자동 재접속, key·ID 유지. 버전 불일치 시 구버전 쪽 표시. 배포 위치 기본 가정: GitHub Releases.
  - signaling 운영: Workers + DO 무료, wrangler 배포, 사용자 도메인 hostname(없으면 workers.dev). 저장: ID ↔ host 공개키, 요청 제한 카운터만. 서버 주소 기본값 + 설정 파일로 변경 가능. STUN stun.cloudflare.com:3478. 서버 장애 시 새 연결·재접속 불가, 진행 중 세션 무관.
  - 로그: host ProgramData\SimpleRemote\logs, viewer %APPDATA%\SimpleRemote\logs, 크기 상한·오래된 것부터 삭제. 화면·클립보드·코드는 로그 금지. host 창에 "기록 폴더 열기".
- 다음 단계: 섹션 6 오류 처리·테스트·완료 기준.
- 섹션 6 제시 → 사용자 답변: "순단 현상에 대한 대비도 스펙에 추가". 나머지 내용(fail closed, 자동 회복 표, 테스트 7계층, 역할 분담, 완료 기준 5개)은 수정 요청 없음.
  - 추가 설계(6-5 순단 대비, 승인 대기): data channel heartbeat 500ms. 1초 무응답 → host 가 눌린 키·버튼 모두 해제. 2초 → viewer "네트워크 불안정" 표시(마지막 화면 흐리게 유지). 5초 또는 viewer 네트워크 변경 감지 → 같은 세션 안에서 ICE restart(세션 key 로 인증한 signaling 메시지, 재인증·허락 없음, viewer 만 시작, host 는 요청만). ICE restart 20초 실패 또는 세션 상태 소실 → 허가증 재접속(D11). 복구 후 key frame 요청, 오래된 클립보드 변경 폐기. host 는 세션 중에도 signaling 등록 유지.
  - 값(500ms, 1s, 2s, 5s, 20s)은 설계 제안값, 실측 후 조정.
  - 테스트 추가: NAT 흉내 테스트에 손실 폭주, 1·5·30초 링크 끊김, viewer IP 변경 시나리오.
  - 완료 기준 추가: 2초 이하 끊김은 재인증 없이 세션 유지, Wi-Fi ↔ 핫스팟 전환은 같은 세션으로 복구, 끊김 중 눌린 키·버튼이 host 에 남지 않음.
  - 확인 필요: 선택할 WebRTC 라이브러리의 ICE restart 지원 (구현 계획 spike 항목).
- D20. 섹션 6 승인 (6-5 순단 대비 포함). 설계 전체 승인 완료 → spec 작성 단계로 이동.
  - spec 경로: `docs/superpowers/specs/2026-09-24-simple-remote-sp1-design.md`.
  - commit 계획: 사용자 Gitflow 규칙(develop 이 기본 브랜치, master 는 배포 기록)에 따라 빈 repo 의 첫 commit 을 `develop` 브랜치에 만든다. spec 과 근거 조사 문서 3건을 함께 commit, 이 진행 기록 파일은 commit 하지 않음.

## 다음 할 일

- spec 승인 완료. 이 진행 기록도 commit 하고 `develop` 을 `https://github.com/dev-exintueri/simple-remote.git`(private, 빈 저장소) 에 push 한다 (사용자 지시). push 는 전역 git 설정을 바꾸지 않고 gh credential helper 를 1회성으로 사용.
- push 후 superpowers:writing-plans 로 SP1 구현 계획을 작성한다.
- writing-plans 진행 상황
  - 계획 분할 방침: spike 결과가 라이브러리 선택을 정하므로 SP1 을 순차 계획 여러 개로 나눈다. 지금은 Phase 0(spec 11절 spike) 계획을 상세히 쓰고, 이후 계획은 spike 결과 뒤에 작성.
  - 개발 환경 확인 결과: WSL 에 Rust 1.88(업데이트 필요), Node 24, gh 로그인. Windows 용 target·linker 없음. Windows 쪽은 Windows 11 Enterprise, 그래픽 2개 노트북(Intel Arc + RTX 4070 Laptop), Windows Rust(msvc) 와 .NET 설치, MSVC linker 는 찾지 못함.
  - 막힌 지점(사람 판단 필요): 이 PC 는 회사 관리 PC 로 보임. SYSTEM 서비스·화면 캡처·입력 주입 테스트를 어디서 돌릴지(이 PC / 개인 PC / VM) 사용자에게 질문했으나 사용자가 질문을 중단함. 답을 받기 전에는 Windows 쪽 환경을 더 조사하지 않는다.
  - 사용자 새 요청: "이 다음 단계부터 Claude Code 앱에서 cloud 로 진행하고 싶은데 가능한가, 어떻게 진행하는 게 좋은가". 클라우드 세션 사실(환경, 설정 전달, 로컬과 오가기)을 공식 문서로 확인 중. writing-plans 는 이 답이 정해질 때까지 보류.
  - 확인 결과 (claude-code-guide 조사, code.claude.com/docs 의 claude-code-on-the-web, cloud-environments 문서 기준):
    - 가능. private org repo 는 Claude GitHub App 을 dev-exintueri org 에 설치하거나 CLI `/web-setup`(로컬 gh token 사용). 시작: claude.ai/code, 데스크톱·모바일 앱 Cloud, CLI `claude --cloud "..."`.
    - 환경: Ubuntu 24.04 x86_64, Rust·Node 기본 설치, setup script(root) 로 추가 설치, 네트워크 기본값 Trusted(crates.io, npm, GitHub 포함). Windows 실행·GPU 없음.
    - 옮겨지지 않음: 이 대화 맥락, 사용자 ~/.claude/CLAUDE.md, 사용자 plugin·skill(superpowers, hid-git), memory, 로컬 미push 변경. 옮겨짐: repo 의 CLAUDE.md, .claude/ 설정·skills.
    - git: 클라우드 세션은 자기 작업 브랜치에만 push → develop 반영은 PR.
    - 불확실: repo settings 의 enabledPlugins 가 클라우드에서 적용되는지 보고 내용끼리 모순 → 사용자 계정 synced plugin 여부를 직접 확인 필요.
  - 제안(사용자 응답 대기): 로컬에서 repo CLAUDE.md(신규 작성 예정) + 인계 메모 작성 후 commit·push → 사용자가 GitHub App 설치·plugin 확인 → 클라우드 세션 시작.
  - 사용자 승인: "1단계 진행해줘". 저장소 루트 `CLAUDE.md` 작성(사용자 전역 규칙 중 이 프로젝트에 필요한 것: 대화 방식, 구현 원칙, 작업 절차, git 규칙), 이 파일 맨 앞에 "클라우드 세션 인계" 절 추가, commit 후 `develop` push.
  - 남은 사람 할 일: dev-exintueri org 에 Claude GitHub App 설치(또는 CLI `/web-setup`), claude.ai 계정에서 superpowers plugin 사용 가능 여부 확인, 클라우드 세션 시작(저장소 `dev-exintueri/simple-remote`, 브랜치 `develop`, 첫 메시지는 인계 절의 문장). 계획의 첫 단계는 spec 11 절 spike 항목(WebRTC 라이브러리, egui, PAKE, SYSTEM DPAPI, MF 인코더, 입력 좌표, WiX, Workers 테스트 도구).
