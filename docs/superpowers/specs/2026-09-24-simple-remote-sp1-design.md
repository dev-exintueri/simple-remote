# simple-remote SP1 설계: Windows 가족 원격 지원 최소 완성품

- 작성일: 2026-09-24
- 상태: 사용자 검토 대기
- 근거 조사 문서
  - `docs/research/2026-09-23-teamviewer-features.md`: TeamViewer 기능 155개 분류 (A 26 / B 37 / C 37 / D 55)
  - `docs/research/2026-09-23-p2p-networking-and-security.md`: P2P 연결, NAT traversal, 보안, 오픈소스 참고 구현
  - `docs/research/2026-09-23-host-platform-windows-macos.md`: 화면 캡처, 인코딩, 입력, DPI, 멀티 모니터, 클립보드, 권한, 배포

이 문서의 사실 주장은 위 조사 문서의 해당 절에 1차 출처가 있다. 조사 근거가 아니라 설계자가 정한 값은 "설계 제안값"으로 표시한다.

---

## 1. 목적

### 1.1 누가, 무엇을 위해 쓰나

- 사용자 본인(기술을 아는 사람)이 컴퓨터를 잘 모르는 가족·지인의 Windows PC 를 원격으로 도와준다.
- 도와주는 쪽은 주로 집 밖(카페, 회사, 휴대폰 핫스팟)에서 노트북으로 접속한다.
- 도움받는 쪽은 대개 가정 공유기 뒤에 있고, 외국에 있을 수도 있다.

### 1.2 성공한 모습

- 가족은 처음 한 번 설치하고, 이후에는 "원격 지원 받기"를 열어 코드를 불러 주고 허락 버튼만 누른다.
- 도와주는 쪽은 포트포워딩 같은 네트워크 설정 없이 접속하고, UAC 창·잠금 화면까지 조작한다.
- 네트워크가 잠깐 끊기거나 가족 PC 가 재부팅돼도 1시간 안에는 다시 허락받지 않고 이어서 돕는다.
- 연결이 안 되면 왜 안 되는지와 무엇을 하면 되는지가 화면에 나온다.

### 1.3 용어

| 용어 | 뜻 |
|---|---|
| host | 제어받는 쪽. 가족 PC |
| viewer | 화면을 보며 제어하는 쪽. 사용자 본인의 노트북 |
| signaling 서버 | 연결을 시작할 때 두 기기가 주소를 교환하도록 돕는 서버. 세션 데이터는 지나가지 않는다 |
| relay | 직접 연결이 안 될 때 세션 데이터를 대신 전달하는 서버. SP1 에는 없다 |
| hole punching | 공유기 뒤의 두 기기가 동시에 패킷을 보내 서로에게 가는 길을 여는 기법 |
| ICE | WebRTC 표준의 경로 찾기 절차. 여러 주소 후보를 동시에 시험해 되는 경로를 고른다 |
| PAKE | 짧은 코드를 네트워크로 보내지 않고 양쪽이 같은 코드를 안다는 것을 증명하며 key 를 합의하는 방식. 엿본 대화로 코드를 오프라인 대입할 수 없다 |
| 재접속 허가증 | 가족이 한 번 허락한 뒤 발급되어, 끊긴 뒤 1시간 안에는 허락 없이 재접속하게 해 주는 기록 |
| DDA | Desktop Duplication API. Windows 화면 복제 API |
| MF | Media Foundation. Windows 내장 미디어 프레임워크 (H.264 인코더·디코더 포함) |
| DPAPI | Windows 계정에 묶인 데이터 암호화 API |

---

## 2. 범위

### 2.1 하위 프로젝트 분해

| 순서 | 하위 프로젝트 | 내용 |
|---|---|---|
| **SP1** | **Windows 가족 원격 지원 최소 완성품 (이 문서)** | 2.2 절 |
| SP2 | 제어권 확장 | 가족 입력 막기, 가족 화면 가리기(`WDA_EXCLUDEFROMCAPTURE`, Windows 10 2004+), 가족 쪽 해제(Ctrl+Alt+Del) |
| SP3 | 클립보드·파일 확장 | 이미지 클립보드, 파일 전송, 파일 복사-붙여넣기(OLE delayed rendering) |
| SP4 | 멀티 모니터·해상도 확장 | 여러 모니터 동시 보기, 원격 해상도 변경과 복구 |
| SP5 | 연결 성공률 확장 | TCP 경로, port 예측, viewer 역방향 연결(DDNS), SP1 실측 결과에 따라 relay |
| SP6 | 모바일 viewer | Android / iOS. UI 도구는 시작할 때 결정 |
| SP7 | macOS viewer | |
| SP8 | macOS host | macOS 권한 승인 흐름 포함 |
| 이후 | TeamViewer 기능 추가 검토 | 조사 문서의 B/C 등급을 하나씩 검토 (채팅, 세션 녹화, 원격 소리, Wake-on-LAN 등) |

각 하위 프로젝트는 spec, 구현 계획, 구현을 따로 거친다.

### 2.2 SP1 포함

| 영역 | 내용 |
|---|---|
| 설치·제거 | host 는 MSI 로 서비스 등록, 제거 시 흔적 없음(검사 스크립트로 확인). viewer 는 설치 없는 단일 실행 파일 |
| 연결 | 9자리 접속 ID, Cloudflare Workers signaling, Cloudflare STUN, 경로 경주(직접, hole punching, IPv6, host 공유기 자동 포트 열기), 경로별 결과 기록, 실패 원인 안내 |
| 보안 | 6자리 일회용 코드 + PAKE, PAKE key 로 연결 정보 보호, 가족 허락 창, 1시간 재접속 허가증(재부팅 포함), 시도 제한, 접속 기록, "원격 제어 중" 표시 |
| 권한 화면 | UAC 창, 잠금 화면, 로그인 화면 보기·조작 |
| 화면 | 모니터 1개씩 보기 + 모니터 전환, H.264 하드웨어 인코딩 우선·내장 소프트웨어 대체, 커서 |
| 해상도·DPI | 배율이 다른 모니터, 화면 맞춤 / 원래 크기, host 해상도·회전·DPI 변경 자동 추종 |
| 입력 | 마우스(휠 포함), 키보드(한/영, 한자 포함), 단축키 보내기 토글, 특수 키 메뉴 |
| 제어권 | 가족 쪽 끊기 버튼과 비상 단축키 |
| 클립보드 | 텍스트 양방향, viewer 원격 창 포커스 시만 |
| 기타 | Windows 10/11 기능 가능 여부 감지·표시, 원격 재부팅 후 자동 재접속, 순단 대비 |

### 2.3 SP1 제외 (비목표)

- relay (직접 연결 실패 시 세션 데이터 중계)
- 여러 viewer 동시 접속
- 가족 확인 없는 무인 접속 (재접속 허가증 1시간 규칙은 예외로 포함)
- 모바일, macOS, Linux
- TLS 검사 proxy 가 있는 회사망의 host
- 코드 서명 (설치 시 SmartScreen 경고를 "추가 정보 → 실행"으로 넘기도록 안내)
- 자동 업데이트
- 안전 모드 재부팅, HDR 전용 처리, YUV 4:4:4
- SP2 이후 항목 전부

---

## 3. 전체 구조

### 3.1 실행 단위

```
[가족 PC]
  host-service (SYSTEM, session 0)        agent 감독, Ctrl+Alt+Del 대행, 재부팅 실행
    └─ 실행 → host-agent (SYSTEM, 활성 콘솔 세션)   연결·보안·캡처·인코딩·입력·클립보드·기록
                 ↕ named pipe (ACL)
              host-ui (가족 권한)                     ID·코드, 허락 창, "원격 제어 중" 막대, 끊기

[사용자 노트북]
  viewer (사용자 권한, 설치 없음)          viewer-core 라이브러리 + 데스크톱 UI

[Cloudflare 무료]
  signaling Worker + Durable Object, STUN   연결 시작 때 주소 교환

host-agent ⇄ viewer : WebRTC 직접 연결 (H.264 영상 + data channel)
host-agent, viewer → signaling : 연결 시작, 재접속, 경로 재탐색 때
```

| 실행 단위 | 맡는 일 | 근거 |
|---|---|---|
| host-service | 부팅 시 시작. 활성 콘솔 세션(로그인 화면 포함)에 host-agent 를 `winlogon.exe` 토큰 기반 SYSTEM 권한으로 띄우고, 세션 변경·agent 종료 시 다시 띄운다. agent 요청으로 `SendSAS`, 재부팅 실행. 세션 중 host-ui 가 없으면 가족 토큰으로 host-ui 를 띄운다. **인터넷 통신을 하지 않는다** | session 0 서비스는 화면·입력에 직접 닿지 못한다. `SendSAS` 는 서비스만 보낼 수 있다. 가장 높은 권한 영역의 공격 면을 줄인다 |
| host-agent | signaling 연결, WebRTC, PAKE·허가증, 캡처·인코딩, 입력 주입, 클립보드, capability 감지, 기록 | UAC 창·로그인 화면을 보고 조작하려면 그 세션의 SYSTEM 프로세스여야 한다. 캡처한 GPU 텍스처를 같은 프로세스에서 인코딩한다 |
| host-ui | 가족에게 보이는 모든 창 | SYSTEM 권한 창을 사용자 화면에 띄우면 권한 상승 통로가 될 수 있다 |
| viewer | 사용자 쪽 전부 | 설치 없이 실행. `viewer-core` 를 분리해 SP6·SP7 이 재사용한다 |
| signaling Worker | ID 발급, 대기 등록, 메시지 전달, 요청 제한 | Cloudflare Workers 기본 언어인 TypeScript. host ID 별 Durable Object 가 대기 WebSocket 을 보관 |

네트워크 연결은 host-agent 에 둔다. 가족이 로그아웃하거나 사용자를 전환하면 콘솔 세션이 바뀌어 agent 가 새로 뜨고 연결이 한 번 끊긴다. 이때 viewer 가 재접속 허가증으로 자동 재접속한다 (5.3 절). 네트워크를 service 에 두는 안은 세션 전환에 강하지만 영상·입력 IPC 가 생기고 session 0 서비스가 인터넷에 노출되어 채택하지 않았다.

### 3.2 코드 묶음 (Rust workspace)

| 경로 | 종류 | 역할 |
|---|---|---|
| `crates/protocol` | lib | data channel 메시지 정의, 메시지 버전, capability 목록 |
| `crates/auth` | lib | PAKE, 기기 key, 재접속 허가증, 재접속 key 합의 |
| `crates/transport` | lib | WebRTC 라이브러리 감싸기, 후보 추가(UPnP/PCP), 경로별 기록, heartbeat, ICE restart |
| `crates/codec` | lib | 인코더·디코더 추상화, MF 구현, 코덱 협상 |
| `crates/platform-win` | lib | 캡처(DDA/GDI), 입력 주입, 클립보드, DPI, 세션·desktop 전환, `SendSAS`, 네트워크 변경 감지 |
| `crates/viewer-core` | lib | viewer 쪽 연결·보안·디코딩 파이프라인·입력/클립보드 처리. UI 없음 |
| `apps/host-service` | bin | |
| `apps/host-agent` | bin | |
| `apps/host-ui` | bin | |
| `apps/viewer` | bin | 데스크톱 UI. `viewer-core` 사용 |
| `signaling/` | TypeScript | Cloudflare Worker + Durable Object |
| `installer/` | WiX | host MSI |
| `tools/` | PowerShell 등 | 제거 흔적 검사 스크립트 |

`platform-win` 의 기능은 trait 뒤에 둔다. SP7·SP8 에서 `platform-mac` 을 같은 trait 으로 추가하고, 테스트에서는 가짜 구현(가짜 화면 소스, 가짜 입력 sink)을 쓴다.

### 3.3 기술 선택

| 항목 | 선택 | 근거 |
|---|---|---|
| 언어 | Rust | SYSTEM 서비스가 인터넷 입력을 처리하므로 메모리 안전성 필요. `windows-rs` 로 DDA·MF·서비스·세션 API 접근. 런타임 없는 단일 실행 파일. 참고 구현 RustDesk 가 Rust (AGPL-3.0 이라 코드는 가져오지 않음) |
| 전송 | WebRTC (ICE, DTLS, SRTP/RTP, data channel), TURN 없음 | 경로 경주와 영상 전송(손실 복구, 혼잡 제어)을 표준에 맡긴다 |
| WebRTC 라이브러리 | `str0m` 0.23.1 (sans-IO, crypto `aws-lc-rs`) | Phase 0 spike 로 확정 (D23). 기능 질문 5개 모두 통과, `webrtc-rs` 0.21 은 손실 뒤 대역폭 추정이 회복하지 못함. 근거: `docs/research/2026-09-23-sp1-phase0-spike-results.md` T4~T7 |
| signaling | Cloudflare Workers + Durable Objects (무료 plan), TypeScript | VM 없이 운영. 무료 plan: 하루 요청 10만 건, WebSocket 메시지 20개 = 요청 1건 |
| STUN | `stun.cloudflare.com:3478` | Cloudflare 문서상 "free and unlimited" |
| UI | egui 후보 | Windows·macOS 공용. 영상 그리기 성능과 한글 입력은 spike 로 확정 |
| 설치 | WiX Toolset 으로 만든 MSI | 표준 제거 경로, 실패 시 되돌리기, 선언적 파일·서비스 관리 |

---

## 4. 기기 신원과 저장

| 대상 | 내용 | 보관 |
|---|---|---|
| host 기기 key | 설치 후 최초 실행 시 Ed25519 key 쌍 생성 | `ProgramData\SimpleRemote\` (SYSTEM 전용 파일 권한) + SYSTEM 계정 범위 DPAPI. `CRYPTPROTECT_LOCAL_MACHINE` 범위는 그 PC 의 모든 사용자가 풀 수 있어 쓰지 않는다 |
| 접속 ID | 9자리 숫자. host 가 처음 signaling 에 등록할 때 서버가 발급하고 host 공개키에 묶는다. host 는 등록마다 서명으로 key 소유를 증명한다 | ID 는 찾아가는 주소일 뿐 신뢰 근거가 아니다 |
| viewer 기기 key | 첫 실행 시 Ed25519 key 쌍 생성, 사용자가 기기 이름을 정한다 (허락 창에 표시) | `%APPDATA%\SimpleRemote\` + 사용자 범위 DPAPI |
| 재접속 허가증 | 5.3 절 | host: ProgramData 저장소. viewer: APPDATA 저장소 |
| 알고 있는 상대 | viewer 는 연결했던 host 의 ID·공개키·표시 이름, host 는 연결했던 viewer 의 공개키·기기 이름 | 같은 저장소 |

업그레이드 시 저장소는 유지하고, 제거 시 삭제한다.

---

## 5. 연결과 보안

### 5.1 signaling 서버의 역할과 신뢰

- 서버가 하는 일: ID 발급, host 대기 등록(모드: 새 연결 받기 / 재접속만 받기 / 세션 중), viewer 요청을 host 에 전달, 두 기기 사이 메시지 전달, 요청 횟수 제한.
- 서버가 저장하는 것: ID 와 host 공개키의 짝, 요청 제한용 카운터.
- 서버가 보는 것: 접속한 IP 주소, 어느 ID 로 연결 요청이 왔는지.
- 서버가 못 보는 것: 코드, 허가증, 주소 후보, 인증서 지문, 화면·입력·클립보드. 두 기기 사이 메시지는 서버에 불투명한 내용이다 (PAKE 메시지 또는 PAKE/재접속 key 로 암호화된 내용).
- 서버를 믿지 않는다: 서버가 메시지를 바꿔치기해도 첫 연결은 PAKE, 재접속은 저장된 기기 공개키로 걸러진다.
- 모든 연결은 `wss://` (TLS, 443) 로 한다.

### 5.2 첫 연결

1. 가족이 host-ui("원격 지원 받기")를 연다. host-agent 가 숫자 6자리 일회용 코드를 만들고 host-ui 가 ID 와 코드를 보여 준다. 이때만 agent 는 "새 연결 받기" 모드로 등록하고, host 공유기에 UPnP/NAT-PMP/PCP 매핑을 요청한다.
2. 사용자가 viewer 에 ID 와 코드를 입력한다. viewer 가 signaling 에 연결 요청을 보낸다.
3. **PAKE 를 가장 먼저** signaling 을 거쳐 수행한다 (SPAKE2, RFC 9382 후보). key 확인 MAC 까지 끝나야 다음으로 간다. 틀리면 여기서 끝나므로 코드를 모르는 사람에게 host 주소 후보가 전달되지 않는다.
4. PAKE 결과 key 로 SDP offer/answer 와 이후 ICE candidate 를 AEAD(인증 암호화)로 보호해 교환한다. 각 쪽 DTLS 인증서 지문은 이 보호된 SDP 안에 있으므로 signaling 서버가 바꿔치기할 수 없다.
5. ICE 경로 경주로 직접 연결을 만들고 DTLS 를 연다. 상대 인증서 지문이 4 단계에서 받은 값과 다르면 끊는다. 암호 채널 안에서 두 기기가 장기 공개키와 기기 이름을 교환하고 각자 서명으로 소유를 증명한다.
6. host-ui 가 허락 창을 띄운다: "[viewer 기기 이름]이 원격 제어를 요청합니다", 처음 보는 기기인지 이전에 연결한 기기인지 표시. 30초 안에 허락하지 않으면 거절.
7. 허락하면 세션을 시작한다: "원격 제어 중" 막대 표시, capability 전송(6.6 절), 영상·입력 시작, 재접속 허가증 발급(5.3 절). 사용한 코드는 폐기하고 새 코드를 만든다.

코드를 아는 사람만 허락 창을 띄울 수 있으므로 허락 창을 반복해서 띄우는 공격이 불가능하다.

### 5.3 재접속 허가증과 재접속

**발급**: 세션이 시작되면 host 가 허가증을 만든다.

| 필드 | 내용 |
|---|---|
| 허가증 번호 | 무작위 값 |
| viewer 공개키 | 이 허가증을 쓸 수 있는 유일한 기기 |
| 상태 | 세션 중 / 대기(끊긴 시각 기록) / 폐기 |
| 만료 | 끊긴 시각 + 1시간. 재접속해서 세션이 다시 시작되면 다음 끊김부터 다시 1시간 |

허가증은 들고만 있으면 쓸 수 있는 표가 아니다. viewer 기기 key 로 인증해야만 쓸 수 있다.

**재접속 흐름**

1. viewer 가 host ID 와 허가증 번호로 signaling 에 재접속 요청을 보낸다.
2. 저장된 서로의 공개키로 상호 인증 key 합의를 한다 (Noise 프로토콜 KK 패턴 후보). host 는 허가증의 viewer 공개키와 일치하는지, 허가증이 대기 상태이고 만료 전인지 확인한다. viewer 는 저장된 host 공개키와 일치하는지 확인한다.
3. 합의한 key 로 5.2 절 4~5 단계와 같이 연결 정보를 교환하고 연결한다.
4. 허락 창 없이 세션을 시작한다. 가족 화면에 "재접속됨" 알림과 "원격 제어 중" 막대를 표시한다.

**자동 재시도**: 네트워크 끊김, host 재부팅, agent 재시작으로 끊긴 경우 viewer 가 자동 재시도한다. 간격은 1, 2, 5, 10, 30초, 이후 60초마다이고 허가증 만료까지 계속한다 (설계 제안값). viewer 가 스스로 끊은 경우는 자동 재시도하지 않고, "최근 연결" 목록에서 버튼으로 재접속한다.

**host 대기 조건**: 대기 상태의 유효 허가증이 있으면 host-ui 가 닫혀 있어도, 재부팅 직후 로그인 화면에서도 agent 는 "재접속만 받기" 모드로 등록한다.

**폐기 조건**

- 가족이 끊기 버튼이나 비상 단축키로 끊었을 때 (그 세션의 허가증 즉시 폐기)
- 끊긴 뒤 1시간이 지났을 때
- 가족이 host-ui 에서 "재접속 허가 모두 취소"를 눌렀을 때
- 프로그램 제거

### 5.4 경로 경주와 경로 기록

- 후보: host 로컬 주소, STUN 으로 알아낸 공인 주소, IPv6, host 공유기 자동 매핑 주소(UPnP IGD / NAT-PMP / PCP). viewer 도 로컬·공인·IPv6 후보를 낸다.
- 공유기 매핑은 대기·세션 중에만 두고 유효 시간을 붙인다 (1시간, 필요하면 갱신. 설계 제안값). 제거가 비정상 종료돼도 유효 시간이 지나면 공유기에서 사라진다.
- relay 후보는 없다. 나중에 relay 를 붙일 때는 후보 하나로 추가한다 (구조 변경 없음).
- 경로 기록 (viewer): 시도마다 후보 쌍별 결과, 최종 선택 경로 종류(로컬 / 공인 / 공유기 매핑 / IPv6), 실패 원인 범주, 양쪽 네트워크 유형 추정. relay 필요 여부 판단의 근거 자료다.
- 예상 실패율: 직접 연결 기법만으로는 연결의 약 10~30% 가 실패한다 (조사 추정. TeamViewer 직접 연결 70%, libp2p 측정 70% ± 7.1%). 휴대폰 데이터망에서는 더 높다. 사용자는 이 실패를 감수하고 실측 후 판단하기로 했다 (D12).

### 5.5 공격 대비

| 위협 | 대응 |
|---|---|
| 코드 추측 | host 가 PAKE 실패마다 다음 시도까지 대기 시간을 2배로 늘린다 (1초부터). 한 코드에 5번 틀리면 코드를 교체한다. 대기 시간은 코드를 교체해도 유지하고, PAKE 가 성공해야 초기화한다. signaling 은 IP 별·ID 별 요청 횟수를 제한한다 |
| signaling 서버의 바꿔치기 | 5.1 절 |
| 코드를 모르는 사람의 주소 수집 | 주소 후보는 PAKE 성공 뒤 암호화해서만 보낸다 |
| 노출 시간 | host 는 host-ui 가 열려 있거나 유효 허가증이 있거나 세션 중일 때만 signaling 에 등록한다. 그 밖에는 인터넷에서 보이지 않는다 |
| 허가증 도용 | 허가증 번호만으로는 쓸 수 없다. viewer 기기 key 인증 필수 |
| 원격에서 비상 단축키 흉내 | agent 는 비상 단축키 조합을 주입하지 않고, 비상 단축키는 실제 키보드 입력만 인정한다 (6.4 절) |
| 암호화 없는 진행 | 그런 코드 경로를 두지 않는다 (8.1 절) |

### 5.6 기록과 실패 안내

- host 접속 기록: 시각, viewer 기기 이름과 key 지문, 결과(성공 / 코드 틀림 / 거절 / 무응답 / 재접속), 사용 경로, 끊긴 이유(가족 끊기 / 비상 단축키 / 네트워크 / 재부팅 / viewer 종료). host-ui 에서 볼 수 있다.
- viewer 실패 안내 범주와 문구 방향

| 범주 | 안내 |
|---|---|
| 상대가 대기 중이 아님 | 가족에게 "원격 지원 받기"를 열어 달라고 하세요 (허가증 만료 포함) |
| 코드 틀림 | 남은 대기 시간 표시, 5회 후 새 코드 필요 |
| 가족이 거절 / 응답 없음 | 가족에게 허락 버튼을 눌러 달라고 하세요 |
| 직접 경로 없음 | 추정 원인(내 쪽 휴대폰 망, UDP 차단, 상대 공유기 매핑 불가)과 "다른 네트워크로 바꿔 보세요" |
| signaling 접속 불가 | 인터넷 연결 확인, 서버 상태 |
| 버전 불일치 | 어느 쪽이 구버전인지와 업데이트 필요 |
| 미디어 기능 없음 | host 가 Windows N/KN 에디션이라 영상 전송 불가 |

### 5.7 알려진 한계

- 재부팅 직후 로그인 화면에서는 가족 권한 창을 띄울 수 없어 "원격 제어 중" 막대는 로그인 직후부터 보인다.
- Rust 의 balanced PAKE 라이브러리(`spake2`, RustCrypto)는 공식 보안 감사를 받지 않았다. RFC 9382 테스트 값으로 검증하고, PAKE 코드를 `crates/auth` 에 격리하며, 감사를 받은 `opaque-ke` 와 구현 계획 단계에서 비교한다.
- 공유기 매핑·IPv6 가 없는 환경에서 양쪽 중 하나라도 hole punching 이 안 되는 NAT 뒤에 있으면 연결되지 않는다.

---

## 6. 화면, 입력, 클립보드

### 6.1 캡처

- 모니터별 DDA 를 기본으로 쓴다. DDA 를 만들 수 없거나(외장 GPU 쪽 출력 등) 프레임이 오지 않으면 GDI `BitBlt` 로 바꾼다. WGC 는 SYSTEM 권한에서 동작하지 않아 쓰지 않는다.
- desktop 전환(UAC, Ctrl+Alt+Del 화면, 잠금), 해상도 변경, 모니터 추가·제거, 전체 화면 전환 때 DDA 는 `DXGI_ERROR_ACCESS_LOST` 를 낸다. 캡처와 입력은 창 없는 전용 thread 에서 상태 기계로 돌리고, 이 경우 버리고 다시 붙인다. 입력 thread 는 `OpenInputDesktop` / `SetThreadDesktop` 으로 현재 입력 desktop 을 따라간다.
- 모니터는 1개씩 보고, viewer 에서 전환한다. 모니터 목록과 각 모니터의 위치·크기(physical pixel)를 viewer 에 보낸다.
- 커서는 모양과 위치를 영상과 따로 보내고 viewer 가 그린다. 메시지 정의에는 "커서를 영상에 합성" 모드도 둔다 (SP8 용).

### 6.2 인코딩과 전송

| 항목 | 설계 |
|---|---|
| 코덱 | H.264 만. 인코딩은 MF H.264 encoder MFT: 하드웨어 MFT 우선, 없거나 실패하면 Windows 내장 소프트웨어 MFT. 디코딩은 MF H.264 decoder (D3D11 가속, 안 되면 소프트웨어) |
| 코덱 협상 | 세션 시작 시 양쪽이 가능한 코덱·profile 목록을 교환해 고른다. SP1 은 H.264 하나지만 구조는 처음부터 둔다. H.264 는 언제나 가능한 기본값으로 유지한다 (모바일 viewer 공통 코덱) |
| 색 변환·크기 | GPU 의 D3D11 video processor 로 BGRA → NV12 변환과 크기 조정을 한 번에 한다. 기본값은 모니터 화면을 비율을 유지한 채 1920x1080 안에 들어가게 줄여 보낸다 (더 작은 모니터는 그대로). viewer 에서 "원본 화질"을 켜면 모니터 해상도 그대로 보낸다 |
| 지연 설정 | `CODECAPI_AVLowLatencyMode` 켜기, B-frame 없음. viewer 가 깨진 프레임을 감지하면 key frame 을 요청하고 host 는 `CODECAPI_AVEncVideoForceKeyFrame` 으로 응답 |
| 혼잡 대응 | WebRTC 라이브러리의 대역폭 추정을 쓰고, 부족하면 bitrate 를 먼저, 그다음 FPS 를 낮춘다. bitrate 는 인코딩 중 `CODECAPI_AVEncCommonMeanBitRate` 로 바꾼다 |
| 목표 | 1920x1080, 30fps (하드웨어 인코더 기준) |

**한계**: MF 내장 디코더는 4:2:0 만 지원해 색 글자가 약간 번질 수 있다. HDR 전용 처리 없음. Windows N/KN 에디션은 MF 코덱이 없어 영상을 보낼 수 없다 (capability 로 감지해 안내).

### 6.3 해상도와 DPI

- host 실행 파일은 manifest 로 Per-Monitor v2 DPI aware 로 선언한다.
- host 의 모든 좌표는 physical pixel 기준 virtual desktop 좌표로 통일한다. origin 은 음수일 수 있다.
- viewer 는 "보고 있는 모니터 안의 physical pixel 좌표"로 입력을 보낸다. host 는 virtual desktop 좌표로 바꾼 뒤 `SendInput` 절대 좌표(0..65535, `MOUSEEVENTF_VIRTUALDESK`)로 변환한다. 반올림 규칙은 문서에 없으므로 주입 후 `GetCursorPos` 로 되읽어 1픽셀 단위로 맞는지 확인하는 테스트를 둔다.
- viewer 보기 방식: 화면 맞춤(비율 유지), 원래 크기(1:1, 스크롤). viewer 도 Per-Monitor v2 DPI aware.
- host 해상도, 회전, DPI 가 바뀌면 캡처를 다시 붙이고 새 모니터 정보를 보내며 viewer 가 자동으로 맞춘다.

### 6.4 입력과 단축키

| 항목 | 설계 |
|---|---|
| 마우스 | 절대 위치, 버튼 5개(왼쪽, 오른쪽, 가운데, X1, X2), 휠은 원래 delta(세로, 가로)를 그대로 보낸다 |
| 키보드 메시지 | 키 하나에 scancode(extended 여부 포함)와 문자(Unicode)·VK 를 모두 담는다. SP1 host 는 scancode 방식으로 주입한다. 문자 정보는 SP6 모바일 화면 키보드처럼 문자만 있는 입력을 위해 둔다 |
| 한국어 키 | 한/영, 한자 키는 host 의 한국어 키보드 종류에 맞춰 변환한다 (오른쪽 Alt 가 `VK_RMENU` 또는 `VK_HANGUL` 로 해석되는 차이) |
| 단축키 보내기 | 켜져 있고 원격 창에 포커스가 있으면 viewer 가 전용 thread 의 `WH_KEYBOARD_LL` hook 으로 Win 키, Alt+Tab 등을 가로채 host 로 보낸다. hook 처리는 제한 시간(최대 1000ms) 안에 끝나야 하므로 무거운 일을 하지 않는다. 포커스를 잃으면 눌린 키를 모두 뗀다 |
| 특수 키 메뉴 | Ctrl+Alt+Del(agent → service → `SendSAS`), Win+L(host 잠금), Ctrl+Shift+Esc, Alt+Tab, Win, Print Screen. Ctrl+Alt+Del 과 Win+L 은 hook 으로 가로챌 수 없어 메뉴로만 보낸다 |
| viewer 예약 조합 | host 로 보내지 않는다. Ctrl+Alt+Enter: 전체 화면 켜기/끄기. Ctrl+Alt+Home: 단축키 보내기 켜기/끄기 |

### 6.5 제어권 표시와 가족 쪽 비상 수단

- "원격 제어 중" 막대: 가족 화면 위쪽 가운데, 항상 위, 끌어서 옮길 수 있음. 내용: "[viewer 기기 이름]이 원격 제어 중" + [끊기] 버튼.
- 비상 단축키: Ctrl+Alt+F8. agent 의 low-level hook 이 `LLKHF_INJECTED` 가 없는 실제 키보드 입력일 때만 인정한다. agent 는 viewer 가 보낸 입력 중 이 조합을 주입하지 않는다.
- 끊기 버튼과 비상 단축키로 끊으면 "가족이 끊음"으로 기록하고 허가증을 폐기한다.

### 6.6 capability 감지와 지원 OS

- 지원 OS: Windows 11 23H2 이상, Windows 10 22H2.
- 세션 시작 시 host 가 보내는 capability: OS 이름·build, MF H.264 인코더 종류(하드웨어 / 소프트웨어 / 없음), 모니터별 캡처 방식(DDA / GDI), 모니터 목록, Ctrl+Alt+Del 정책 상태, 이후 SP 가 쓸 기능 플래그(예: `WDA_EXCLUDEFROMCAPTURE` 가능 여부).
- viewer 는 쓸 수 없는 기능을 메뉴에서 비활성화하고 이유를 보여 준다. 기능별 API 호출은 capability 확인 뒤에만 한다.

### 6.7 클립보드 (텍스트)

- 형식: `CF_UNICODETEXT`.
- viewer 원격 창에 포커스가 있을 때만 양방향 동기화. 포커스를 얻는 순간 viewer 클립보드를 host 로 보낸다.
- 되돌림 방지: 원격에서 받아 설정한 내용에 전용 private format 표식을 붙이고, 표식이 있는 변경은 되돌려 보내지 않는다.
- 원격에서 받은 내용에는 `ExcludeClipboardContentFromMonitorProcessing` 형식을 함께 넣어 클립보드 기록(Win+V)과 클라우드 클립보드 동기화 양쪽에서 빠지게 한다.
- 상한 1 MiB (설계 제안값). 넘으면 보내지 않고 viewer 에 표시한다.
- 클립보드가 다른 프로그램에 잠겨 있으면 짧게 몇 번 재시도하고 건너뛴다.

### 6.8 원격 재부팅

- viewer 메뉴 "원격 재부팅" → 확인 → agent 가 service 에 요청 → service 가 재부팅. 허가증은 "대기" 상태가 되고 부팅 후 agent 가 로그인 화면 세션에서 "재접속만 받기"로 등록, viewer 가 자동 재접속한다.
- 안전 모드 재부팅은 제외.

---

## 7. 설치, 제거, 운영

### 7.1 host 설치 (MSI)

| 설치 대상 | 내용 | 제거 시 |
|---|---|---|
| 실행 파일 | `Program Files\SimpleRemote\` 에 host-service, host-agent, host-ui | 삭제 |
| 서비스 | 자동 시작, LocalSystem, 실패 시 재시작 | 중지·삭제 |
| 데이터 | `ProgramData\SimpleRemote\` (SYSTEM 전용 권한): 기기 key, 허가증, 알고 있는 상대, 접속 기록, 설정, 로그 | 삭제. 업그레이드 시 유지 |
| 방화벽 규칙 | host-agent 프로그램 한정 수신 허용 | 삭제 |
| `SoftwareSASGeneration` 정책 | 원래 값을 기록하고 서비스 허용 값으로 설정 | 원래 값 복원 (원래 없었으면 삭제) |
| 바로가기 | 바탕화면·시작 메뉴 "원격 지원 받기" | 삭제 |
| 공유기 매핑 | 설치 시 만들지 않음 (5.4 절) | 제거 시작 시 해제 시도, 실패해도 유효 시간 후 소멸 |

- host-ui 는 로그인 시 자동 실행하지 않고 트레이 아이콘도 없다. 세션 중 host-ui 가 없으면 service 가 띄운다.
- 첫 설치: 링크 전달 → 가족이 MSI 실행 → SmartScreen 경고에서 "추가 정보 → 실행" → UAC "예" → 바로가기 생성. 전화 안내를 전제로 한다.
- 제거 순서: 진행 중 세션 종료 → 공유기 매핑 해제 → signaling 등록 해제(가능하면) → 서비스 중지·삭제 → 파일·데이터 삭제 → 방화벽 규칙 삭제 → 정책 복원 → 바로가기 삭제.

### 7.2 viewer

- 설치 없는 단일 실행 파일. 데이터는 `%APPDATA%\SimpleRemote\`.
- 메뉴 "내 데이터 모두 삭제".

### 7.3 제거 흔적 검사

- `tools/` 에 PowerShell 검사 스크립트를 둔다. 깨끗한 Windows VM 에서 설치 전 상태를 기록하고, 설치 → 연결 → 제거 뒤 다시 기록해 비교한다.
- 비교 대상: 서비스 목록, `Program Files`·`ProgramData`·사용자 프로필 아래 관련 경로, 관련 레지스트리(Uninstall, Services, 정책 값), 방화벽 규칙, 바로가기, 예약 작업.
- 차이가 1건이라도 있으면 실패. 출시마다 실행한다.

### 7.4 업데이트와 버전

- SP1 은 자동 업데이트가 없다.
- 업데이트 방법: 사용자가 원격 제어로 가족 PC 브라우저에서 새 MSI 를 받아 실행한다 (UAC 도 원격 조작 가능). 서비스 재시작으로 끊기면 허가증으로 자동 재접속한다. 기기 key 와 ID 는 유지된다.
- 연결 시 protocol 버전을 교환하고, 호환되지 않으면 어느 쪽이 구버전인지 표시한다.
- 설치 파일 배포 위치: GitHub Releases (기본 가정).

### 7.5 signaling 운영

- Cloudflare Workers + Durable Objects 무료 plan. 배포는 `wrangler`. 주소는 사용자 도메인의 hostname, 없으면 `workers.dev`.
- host 와 viewer 는 서버 주소 기본값을 내장하고, 설정 파일(host: ProgramData, viewer: APPDATA)로 바꿀 수 있다.
- 서버 장애 시 새 연결, 재접속, 경로 재탐색이 불가능하다. 이미 연결된 세션은 영향이 없다.

### 7.6 로그

- host: `ProgramData\SimpleRemote\logs`, viewer: `%APPDATA%\SimpleRemote\logs`. 파일 크기 상한을 두고 오래된 것부터 지운다.
- 화면 내용, 클립보드 내용, 코드, key 는 로그에 남기지 않는다.
- host-ui 에 "기록 폴더 열기".

---

## 8. 오류 처리

### 8.1 보안 경로: 실패하면 막는다

다음 중 하나라도 실패하면 연결을 거절하고 이유를 기록한다. 암호화 없이 진행하는 코드 경로는 두지 않는다.

- PAKE 실패, key 확인 MAC 불일치
- 기기 key 서명 확인 실패, 저장된 공개키와 불일치
- DTLS 인증서 지문 불일치
- 허가증 무효·만료·폐기, 허가증의 viewer 공개키 불일치
- 허락 창 무응답, 거절
- 새 연결인데 host-ui 와 통신할 수 없음 (허락 창을 띄울 수 없음)
- protocol 버전 불호환

### 8.2 그 밖의 오류: 스스로 회복한다

| 오류 | 회복 |
|---|---|
| host-agent 종료 | service 가 다시 띄움. viewer 가 허가증으로 자동 재접속 |
| host-service 종료 | Windows 서비스 관리자가 재시작 (설치 시 복구 설정) |
| host-ui 종료 (세션 중) | service 가 다시 띄움 |
| 하드웨어 인코더 오류, GPU 재설정 | 내장 소프트웨어 인코더로 전환 |
| DDA 실패 | GDI 캡처로 전환 |
| viewer 디코딩 오류 | key frame 요청, 계속되면 디코더 재생성 |
| 클립보드 잠김 | 짧게 재시도 후 건너뜀 |
| 경로 끊김 | 8.3 절 |

viewer 상태 표시: 연결 중 / 연결됨(경로 종류) / 불안정 / 경로 재탐색 중 / 재접속 중(허가증 남은 시간) / 실패(원인과 조치).

### 8.3 순단 대비

연결 안에서 viewer 와 host 가 500ms 마다 heartbeat 를 주고받고, 끊긴 시간에 따라 단계적으로 대응한다. 시간 값은 설계 제안값이며 실측 후 조정한다.

| 끊긴 시간 | 동작 |
|---|---|
| 1초 | host 가 viewer 입력으로 눌린 상태인 키와 마우스 버튼을 모두 뗀다 (키가 눌린 채 남거나 드래그가 계속되는 것 방지) |
| 2초 | viewer 에 "네트워크 불안정" 표시. 마지막 화면을 흐리게 유지 |
| 5초, 또는 viewer 가 자기 네트워크 변경을 감지했을 때 | 같은 세션 안에서 ICE restart. 새 후보를 signaling 으로 교환하되 이번 세션의 key 로 보호한다. PAKE, 허락 창, 허가증이 필요 없다 |
| ICE restart 를 시작하고 20초 안에 경로를 못 찾음, 또는 세션 상태 소실 (agent 재시작 등) | 5.3 절 재접속 흐름으로 전환 |

- ICE restart 는 viewer 만 시작한다. host 가 자기 네트워크 변경을 감지하면 signaling 으로 viewer 에게 재탐색을 요청만 한다.
- host 는 세션 중에도 signaling 등록("세션 중" 모드)을 유지한다.
- 복구 후 viewer 는 key frame 을 요청한다. 끊긴 동안 쌓인 클립보드 변경은 버린다.
- 네트워크 변경 감지: Windows 네트워크 인터페이스 변경 알림을 쓴다 (`platform-win`).

---

## 9. 테스트

### 9.1 계층

| 계층 | 내용 | 실행 위치 |
|---|---|---|
| 단위 | 메시지 인코딩, PAKE(RFC 9382 테스트 값), 허가증 발급·만료·폐기·재시작 후 복원, 재접속 key 합의, 좌표 변환, 한국어 키 변환표, 클립보드 되돌림 방지, 실패 원인 분류, 대기 시간 증가·순단 단계(가짜 시계) | 자동, Linux·Windows |
| signaling | ID 발급·key 소유 증명, 대기 모드, 메시지 전달, 요청 제한 | 자동, Cloudflare Workers 로컬 테스트 도구 |
| 연결 통합 | 한 기기 안에서 viewer-core 와 host-agent 핵심 로직을 가짜 화면 소스·가짜 입력 sink 로 연결. 첫 연결, 허락, 영상·입력 전달, 재접속, 폐기, 틀린 코드 제한, 지문 바꿔치기 차단 | 자동, Linux |
| NAT·순단 흉내 | Linux network namespace 로 NAT 유형(endpoint-independent / endpoint-dependent mapping), UDP 차단, 손실 폭주, 1·5·30초 링크 끊김, viewer IP 변경을 흉내 내 경로 경주, 실패 분류, ICE restart, 재접속 전환을 확인 | 자동, Linux |
| Windows 플랫폼 | UAC·잠금 전환 중 캡처 재부착, 배율이 다른 모니터에서 클릭 좌표 1픽셀 확인, 한국어 키 주입, 클립보드 형식, `SendSAS`, MF 인코더 선택 | 실제 Windows 화면 세션 (개발 PC 의 Windows 또는 VM) |
| 설치·제거 | 7.3 절 흔적 검사 | 깨끗한 Windows VM |
| 실사용 점검 | 서로 다른 두 네트워크(예: viewer 휴대폰 핫스팟, host 가정 공유기)에서 체크리스트 수행, 경로 기록 수집 | 사람 |

### 9.2 사람과 에이전트의 역할

| 사람이 할 일 | 에이전트가 할 일 |
|---|---|
| Cloudflare 계정 로그인(`wrangler login` 은 브라우저 필요), 도메인 연결 | 코드, 테스트, 검사 스크립트 작성 |
| 테스트용 Windows VM 준비 | WSL 에서 Windows 대상 교차 빌드 |
| 서비스 설치 테스트의 UAC 승인 | 단위·통합·NAT·순단 흉내 테스트 실행 |
| 실사용 점검 (두 번째 PC 또는 가족 PC, 서로 다른 네트워크) | 관리자 권한이 필요 없는 Windows 쪽 실행 테스트 |
| GitHub 저장소와 Releases 준비 (원할 경우) | 경로 기록 분석과 relay 필요 여부 보고 |

---

## 10. 완료 기준

1. 기능: 2.2 절 SP1 포함 항목이 실사용 점검 체크리스트를 모두 통과한다.
2. 연결: viewer(집 밖 Wi-Fi)와 host(가정 공유기) 조합에서 연결된다. 실패하면 원인 범주와 조치가 표시된다.
3. 성능 (설계 제안 목표): 하드웨어 인코더가 있는 host 에서 1920x1080, 30fps. host 캡처부터 viewer 표시까지 처리 지연(네트워크 전송 시간 제외) 100ms 이하. 프레임에 넣은 시각 정보로 viewer 가 측정해 표시한다.
4. 보안: 다음 테스트를 모두 통과한다.
   - 한 코드에 5번 틀리면 코드가 교체된다.
   - DTLS 인증서 지문을 바꿔치기하면 연결이 차단된다.
   - 폐기되거나 만료된 허가증은 거부된다.
   - 다른 기기 key 로 허가증을 쓰면 거부된다.
   - viewer 가 보낸 비상 단축키는 무시된다.
5. 순단:
   - 2초 이하 끊김은 재인증 없이 세션이 유지된다.
   - viewer 가 Wi-Fi 와 핫스팟 사이를 전환해도 같은 세션으로 복구된다.
   - 끊김 중 눌려 있던 키와 버튼이 host 에 남지 않는다.
6. 제거: 흔적 검사에서 차이 0건.

---

## 11. 구현 계획 단계에서 먼저 확인할 것 (spike)

| 항목 | 확인할 내용 | 영향 |
|---|---|---|
| WebRTC 라이브러리 (`str0m` / `webrtc-rs`) | 외부 후보(UPnP 매핑 주소) 추가, ICE restart, H.264 RTP packetization, data channel(신뢰·비신뢰), 대역폭 추정, Windows IPv6 후보 동작 | `crates/transport`, `crates/codec` |
| UI (egui) | 1080p30 영상 텍스처 갱신 지연, 한글 IME 입력, Per-Monitor v2 DPI | `apps/viewer`, `apps/host-ui` |
| PAKE | `spake2` 와 `opaque-ke` 비교, RFC 9382 테스트 값 | `crates/auth` |
| 재접속 key 합의 | Noise KK 구현 crate 선택 | `crates/auth` |
| SYSTEM 계정 DPAPI | LocalSystem 으로 암호화한 데이터를 다른 계정이 풀 수 없는지 | 4 절 저장소 |
| MF 인코더 | SYSTEM 권한 agent(사용자 세션)에서 하드웨어 MFT 생성, D3D11 texture 입력 | `crates/codec` |
| 입력 좌표 | `SendInput` 절대 좌표 반올림 되읽기 테스트 | `crates/platform-win` |
| WiX Toolset | 현재 버전과 이용 조건, 서비스·방화벽·정책 복원 구성 | `installer/` |
| Cloudflare Workers 로컬 테스트 도구 | Durable Object·WebSocket 을 포함한 테스트 방법 | `signaling/` |

---

## 12. 결정 기록

| 번호 | 결정 | 근거 |
|---|---|---|
| D1 | 최우선 용도: 가족·지인 원격 지원 (attended) | 사용자 선택 |
| D2 | 언어는 문제에 가장 적합한 것을 근거와 함께 추천 | 사용자 선택 → D13 에서 Rust |
| D3 | host 가 외국·공유기 환경이어도 연결 수고가 없어야 함 | 사용자 요구 |
| D4 | 서버는 무료 tier, Cloudflare hostname 가능. 서버는 첫 연결을 돕는 역할만 | 사용자 답변 |
| D5 | relay 제외 (트래픽 비용) | 사용자 결정, D12 로 대체 |
| D6 | viewer 는 주로 집 밖 → host 공유기 매핑 + hole punching + IPv6 동시 시도 | 사용자 답변 |
| D7 | 첫 버전은 host·viewer 모두 Windows, macOS 는 나중. OS 의존 부분 분리 | 사용자 답변 |
| D8 | Windows 11 위주, Windows 10 호환. 기능 가능 여부를 감지해 viewer 에 표시 | 사용자 요구 |
| D9 | 처음부터 설치형 서비스, 깨끗한 제거 | 사용자 선택. UAC·잠금 화면 조작은 SYSTEM 구조에서만 가능 |
| D10 | 제어권: 가족 즉시 되찾기, 입력 막기·화면 가리기(SP2). 기능별 권한 대신 "원격 제어 중" 명확 표시 | 사용자 답변 |
| D11 | 연결은 매번 가족 확인, 끊긴 뒤 1시간 안 재접속(재부팅 포함)은 확인 없이 허용. 가족이 직접 끊으면 허가증 폐기 | 사용자 답변 + 설계 제안(이의 없음) |
| D12 | relay 없이 시작, 경로별 결과를 기록해 실측 후 판단 | 사용자 선택. 조사상 실패 약 10~30% 추정, Oracle Always Free 로 비용 없는 relay 가능성 확인 |
| D13 | Rust + WebRTC 로 새로 만들기 | 사용자 선택. QUIC 직접 구현과 RustDesk fork 대비 |
| D14 | 하위 프로젝트 분해와 SP1 범위, 코드 서명 제외 | 사용자 승인 |
| D15 | host 3 프로세스 구조, 네트워크는 agent, TypeScript signaling | 사용자 승인 |
| D16 | 모바일 viewer 는 SP6, `viewer-core` 분리와 H.264 기본값 유지 | 사용자 선택 |
| D17 | 연결·보안 흐름 (5 절) | 사용자 승인 |
| D18 | 화면·입력·클립보드 (6 절) | 사용자 승인 |
| D19 | 설치·제거·운영 (7 절) | 사용자 승인 |
| D20 | 오류 처리·테스트·완료 기준, 순단 대비 추가 (8~10 절) | 사용자 승인 + 사용자 요구(순단) |
| D23 | WebRTC 라이브러리는 `str0m` | Phase 0 spike 결과 (T7), 사용자 승인 |
