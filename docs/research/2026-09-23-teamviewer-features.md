# TeamViewer 기능 조사 (무엇을 가져올지 정하기 위한 자료)

- 조회일: 2026-09-23 (모든 출처를 이 날짜에 열어 확인)
- 작성 상태: 완료
- 목적: P2P 원격 제어 프로그램(Windows host 우선, macOS 는 나중)을 설계하기 전에, TeamViewer 가 제공하는 기능을 빠짐없이 모아 무엇을 가져올지 정하는 근거로 쓴다.

## 0. 먼저 읽을 것

### 조사 방법

- 출처는 TeamViewer 가 직접 운영하는 곳만 썼다: teamviewer.com (제품 페이지, Knowledge Base(KB), 법적 제품 설명(product description), 보도자료, Trust Center, 보안 PDF), community.teamviewer.com (공식 공지와 버전별 Change Log).
- tier(요금제) 값은 product description 페이지의 원본 HTML 표에서 읽었다. 요약 도구가 체크 표시를 잘못 옮긴 사례가 있어서다. product description 은 계약 문서라서 KB 와 다를 때는 product description 을 기준으로 삼았다. 서로 다른 곳은 10장에 모았다.
- 2019년 v14 공식 매뉴얼(PDF)에서만 확인한 세부 항목은 "(v14)"로 표시했다. 1차 출처지만 오래돼 현재 화면과 다를 수 있다.
- 2차 출처(블로그, 리뷰 사이트)에만 있는 항목은 "미확인(2차 출처)"로 표시했다. 1차 출처에서 아예 못 찾은 항목은 "못 찾음"으로 적었다. 표시가 없는 행은 모두 1차 출처에서 확인한 것이다.

### 분류 기준 (작은 규모로 직접 만드는 P2P 도구 기준)

| 분류 | 뜻 |
|---|---|
| A | 핵심. 거의 모든 원격 제어 도구에 있고, 쓸 만한 MVP 에 필요하다 |
| B | 흔함. MVP 직후 바로 기대되는 기능이다 |
| C | 고급. 가치는 있지만 driver, 별도 서버, 별도 client 같은 큰 구현 부담이 있다 |
| D | 엔터프라이즈이거나 범위 밖이다. 계정이나 중앙 관리 서버가 있어야 하거나, TeamViewer 스스로 접는 기능이거나, 현재 대상 OS 밖이다 (행마다 이유를 적음) |

분류는 이 조사의 판단이다. 사용자가 직접 꼽은 우선순위(해상도 처리 전부, 제어권 관리, 단축키, clipboard)는 11장에서 따로 다룬다.

### tier 약어

Free(개인 무료) / RA(Remote Access) / Bus(Business) / Prem(Premium) / Corp(Corporate) / Tensor / ONE(TeamViewer ONE). "Bus+"는 Business 이상이라는 뜻이다. 각 tier 의 내용은 1장에 있다.

## 핵심 요약

| 영역 | A | B | C | D | 합계 |
|---|---|---|---|---|---|
| 2. 연결 (C) | 8 | 8 | 7 | 10 | 33 |
| 3. 보안 (S) | 8 | 6 | 2 | 13 | 29 |
| 4. 세션 중 기능 (F) | 1 | 9 | 14 | 4 | 28 |
| 5. 제어권 관리 (CT) | 2 | 4 | 5 | 1 | 12 |
| 6. 키보드·단축키 (K) | 2 | 3 | 1 | 1 | 7 |
| 7. 화면 (V) | 4 | 5 | 4 | 0 | 13 |
| 8. 관리·엔터프라이즈 (M) | 0 | 0 | 2 | 20 | 22 |
| 9. 플랫폼 (P) | 1 | 2 | 2 | 6 | 11 |
| 합계 | 26 | 37 | 37 | 55 | 155 |

(2~9장 표의 행을 센 값이다. 한 행이 TeamViewer 기능 하나이고, 같은 기능을 두 번 세지 않았다.)

설계 결정에 바로 쓰이는 사실:

1. **relay 서버는 선택이 아니다.** TeamViewer 도 master server 에서 handshake 를 한 뒤 "70 percent" 만 UDP/TCP direct 로 붙고, 나머지는 router 망을 거쳐 TCP 나 HTTP tunneling 으로 간다. 포트도 5938 → 443 → 80 순서로 fallback 한다 ([sec-statement], [ports]). "연결 초기화만 돕는 서버"로 끝나지 않고, 약 30% 연결에는 relay 가 필요하다는 근거다.
2. **보안 기준선이 구체적으로 공개돼 있다.** 15.73 부터 mutually authenticated TLS 1.3 + Diffie-Hellman 기반 PFS(Perfect Forward Secrecy: 나중에 key 가 새어도 지난 세션은 풀 수 없게 하는 성질) + AES-256-GCM 이고, relay 는 내용을 풀 수 없다. password 는 SRP v6(password 자체를 보내지 않는 login protocol)으로 확인한다. 실패하면 대기 시간이 지수로 늘어 "17 hours for 24 attempts"가 걸린다 ([sec-statement], [brute-force]). 반대로 TeamViewer 의 LAN 모드는 "regular symmetric encryption without public/private key exchange"라서 따라 하면 안 된다 ([lan]).
3. **2026년 TeamViewer 보안 사고는 암호가 아니라 동의·승인 단계에서 났다.** "Allow after confirmation" 우회(TV-2026-1003), macOS unattended 경로의 연결 2FA 우회(TV-2026-1007)가 그 예다 ([bulletins]). 동의 확인은 host 쪽에서, 모든 연결 경로(attended, unattended, LAN)에 똑같이 걸어야 한다.
4. **제어권 모델은 "권한 수준 + 기능별 권한 + host 쪽 비상 탈출"로 이뤄진다.** 권한 수준은 Full access / Confirm all / View and show / Custom / Deny 다. Custom 은 기능마다 Allowed / Upon confirmation / Denied 를 고르고, 양쪽 설정이 다르면 더 제한적인 쪽을 따른다(v14). host 사용자는 Ctrl+Alt+F8(2026-09 부터 바꿀 수 있음)로 세션을 즉시 끊고, Ctrl+Alt+Del 로 입력 차단을 풀 수 있다 ([policy-settings], [v14-manual], [d151662], [black-screen]).
5. **clipboard 는 text 와 image 만 동기화하고 파일 clipboard 는 없다.** 양쪽 모두 켜야 동작한다. 15.73.3 부터는 supporter 쪽 세션 창이 focus 일 때만 동기화해서, 로컬에서 복사한 내용이 새지 않게 막는다. 붙여넣기가 막힌 입력칸에는 "Paste as keystrokes"를 쓴다 ([clipboard], [d142871], [paste-keys]).
6. **driver 가 필요한 기능은 TeamViewer 에서도 비싸다.** VPN, smart card redirection, 가상 모니터는 Windows 에서만 되고, 원격 인쇄는 Windows 와 macOS 사이에서만 된다. 이 중 VPN, 원격 인쇄, smart card 는 driver 방식이라 Windows ARM native 판에서는 빠진다 ([vpn], [sec-key], [virtual-monitor], [printing], [arm]). 모두 C 나 D 로 분류했다.
7. **macOS 는 권한 흐름이 설계의 중심이다.** Screen Recording, Accessibility, Full Disk Access, Remote Desktop 네 가지 권한이 필요하고 "cannot be granted remotely"다. Screen Recording 은 MDM(Mobile Device Management, 기업 기기 관리 도구)으로도 대신 승인할 수 없다 ([mac], [mac-27-mdm]).

## 1. 라이선스 tier 요약

기준 출처: TeamViewer product description ([PD-remote], [PD-tensor], [PD-one]). 요금 페이지 [pricing] 는 가격이 JavaScript 로 로드돼 금액은 확인하지 못했다. 요금 페이지 본문에서는 "연간 구독만 제공"과 "Only an expert needs a license"(연결을 거는 쪽만 license 필요)를 확인했다.

- "channel"은 동시에 걸 수 있는 outgoing 연결 수다.
- "sessions/ch"는 channel 하나에서 tab 으로 여는 session 수다.

| Tier | 사용자 수 | channel / sessions per ch | 관리 기기 수 | 이 tier 에서 처음 열리는 주요 기능 |
|---|---|---|---|---|
| Free (개인·비상업) | 0 | 1 / 1 | 3 | web client, unattended access, 2FA, multi-monitor. mobile 은 "Limited". VPN, WoL, black screen, 원격 인쇄, API, policy 없음 |
| Remote Access | 1 | 1 / 3 | 계약별 | Windows/macOS/Linux 로만 연결. 원격 인쇄, WoL, black screen, terminal server 지원, 파일 전송 queue |
| Business | 1 | 1 / 3 | 200 | custom branding, session recording, VPN, VoIP/video/chat, switch sides, Service Queue, policy 5개, device group, Web API |
| Premium | 15 | 1 (최대 5) / 10 | 300 | session handover, Remote Terminal, Linux headless, user management, 접속 보고서, 고객 만족도 평가, policy 10개 |
| Corporate | 30 | 3 (최대 10) / 15 | 500 | Mass deployment(MSI), 기기 접속 보고서, AD Connector(수동 동기화), policy 15개. SSO 와 Conditional Access 는 없음 |
| Tensor (Lite/Basic/Pro/Unlimited 등) | 5명부터 | 계약별 / 15 | 계약별 | SSO(Basic 이상 포함), Conditional Access(Pro/Unlimited), audit log + API, multitenancy, policy 60개 |
| TeamViewer ONE (Standard ~ Enterprise Ultimate) | 계약별 | 사용자당 1 / 무제한 | 100 ~ 5,000 이상 | Remote + AI + RMM(원격 모니터링·관리) + DEX(사용자 경험 분석)를 한 license 로 묶음. 2025-05 출시 ([press-one]) |

현재 제품 구성은 다음과 같다. 예전 데스크톱 UI 는 "TeamViewer (Classic)"이라 부르고, Linux 에는 아직 Classic 만 있다 ([supported-os]).

- TeamViewer Remote: Free / RA / Bus / Prem / Corp. 예전 이름은 "TeamViewer Core".
- TeamViewer Tensor.
- TeamViewer ONE.
- 별도로 파는 제품: DEX, Remote Management, Frontline, Assist AR, IoT, Engage, Meeting. Pilot 은 갱신만 받는다.

## 2. 연결 종류와 접속 방법

| ID | 기능 | 설명 | Tier | 분류 | 출처 |
|---|---|---|---|---|---|
| C-01 | Remote control | 상대 화면을 보고 키보드·마우스로 조작한다. 연결 mode 를 지정하지 않으면 이 mode 가 기본이다 | 모든 tier (RA 는 계약한 기기 수까지) | A | [cli], [PD-remote] |
| C-02 | Direct 연결 + relay fallback | master server 에서 handshake 후 "a direct connection via UDP or TCP is established in 70 percent of all cases". 나머지는 router 망을 거쳐 TCP 나 HTTP tunneling 으로 간다 | - | A (P2P 도구의 핵심 경로. 30% 는 relay 가 필요) | [sec-statement] |
| C-03 | Port fallback | 5938 TCP/UDP 가 기본, 443 TCP 가 fallback, 80 TCP 는 "last resort". inbound port 는 필요 없다. iOS 앱은 443 을 쓰지 않는다 | - | A (방화벽 환경의 연결 성공률을 좌우함) | [ports] |
| C-04 | LAN 연결 (IP 직접 입력) | "Incoming LAN connections"를 Accept 나 Accept exclusively 로 두고 IP 나 호스트 이름으로 연결한다. 기본은 꺼져 있다. "No TeamViewer servers are involved… regular symmetric encryption without public/private key exchange is used"이고, connection report 에도 남지 않는다. Accept exclusively 로 두면 인터넷 연결을 받지 않는다 | 모든 Remote 사용자 | A (서버 없는 P2P 의 가장 단순한 경로. 다만 key exchange 를 생략하는 방식은 따라 하지 않는다) | [lan] |
| C-05 | ID + password (attended) | 상대 ID 와 password 로 연결한다. KB 는 session link 로 옮기라고 권하지만, 15.67.3 에서 Remote Support 화면을 다시 ID/password 배치로 되돌렸다 | 모든 Remote 사용자 | A | [id-pw], [d141320] |
| C-06 | Random password | 강도는 6자 / 8자 / 10자(특수문자 포함) / Disabled 중에서 고른다. 세션 뒤 처리는 Keep current / Generate new / Deactivate / Show confirmation 중 하나이고, 15.51.5 부터 기본값이 "Generate new"다 | 모든 사용자 | A | [random-pw], [d134409] |
| C-07 | One-Time password | incoming 세션 한 번만 유효하고 세션이 끝나면 만료된다. Windows Classic 전용 | 문서에 없음 | B | [otp] |
| C-08 | Session link / session code | 링크, code, 이메일 초대로 공유한다. 고객은 QuickSupport 에 code 를 넣고 "Allow access"를 누른다. 고객은 계정이 필요 없다. session code 는 "public key cryptography" 로 인증한다 | 모든 Remote 사용자 | B (signaling 서버에 session 을 등록하는 기능이 필요) | [attended], [sec-pdf] |
| C-09 | QuickSupport (설치 없는 host) | 실행 파일 하나로, 설치나 관리자 권한 없이 돈다. incoming 만 받고 outgoing 은 못 하며, unattended 도 지원하지 않는다. Win/mac/Linux/Android/iOS | 모든 사용자 | A (attended 지원의 기본 형태) | [qs-classic], [qs] |
| C-10 | Host (상시 설치 host) | 24시간 unattended 접속용으로 설치하는 구성이다. 계정 할당이 필요하다. QuickSupport 세션 중에 Host 를 원격 설치할 수 있다 | Host 는 모든 사용자, 원격 설치는 Bus+ | B (주 용도가 "내 PC 원격 접속"이면 A 로 올려야 함) | [host], [unattended], [PD-remote] |
| C-11 | Personal password (고정 password) | unattended 용 고정 password 로, Host/Full client 에서만 쓴다. 15.51.5 부터 이 설정에 관리자 권한이 필요하고, 15.61.3 부터 상태 표시줄에 경고가 뜨며, 15.65.4 부터 policy 로 금지할 수 있다 | 모든 사용자 | B | [personal-pw], [d134409], [d139571], [d140812] |
| C-12 | Easy Access | 계정에 할당된 기기에 ID/password 없이 unattended 로 연결한다. public key 방식이다 | Classic 문서는 모든 사용자, 배포 가이드는 Corp/Tensor (문서끼리 다름) | C (계정 서버 대신 "기기끼리 key 를 한 번 pairing 해서 신뢰"하는 방식으로 구현 가능) | [easy-access], [easy-access-deploy], [sec-pdf] |
| C-13 | Managed / bookmarked device, 링크로 기기 추가 | 계정에 할당된 managed 기기만 unattended 로 연결할 수 있고, bookmark 한 기기는 안 된다 | 관리 기기 수: Free 3 / Bus 200 / Prem 300 / Corp 500 | D (계정·기기 관리 서버가 있어야 함) | [bookmarked], [PD-remote] |
| C-14 | 저장한 기기 목록 (Device dock) | 저장한 기기로 바로 연결하고, Remote Terminal 이나 VPN 으로도 열 수 있다 | 모든 사용자 | B (로컬 주소록 수준이면 쉬움) | [dock] |
| C-15 | Portable (설치 없는 controller) | USB 등에서 바로 실행하는 Full version | Prem/Corp/Tensor | B | [portable] |
| C-16 | 여러 기기 동시 연결, tab | channel 하나 안에서 여러 session 을 tab 으로 연다. channel 당 session 수는 Free 1 / RA 3 / Bus 3 / Prem 10 / Corp 15. 15.73.3 부터 기기 5대를 골라 한 번에 연결할 수 있고, macOS 는 15.76.3 에 "Open new connections in tabs"가 생겼다 | tier 별 | B | [PD-remote], [toolbar], [d142871], [d143409] |
| C-17 | File transfer 전용 연결 | 화면 없이 파일만 주고받는 mode (`-m fileTransfer`). "secure (encrypted), fast, and direct (peer-to-peer)" | 모든 tier (Free 는 한 번에 파일 1개) | B | [ft-session], [cli] |
| C-18 | Remote Terminal | 화면 제어 없이 Windows command prompt 에 접속한다. Easy Access 가 필요하고 Windows 10 1809 이상이다 | Prem/Corp/Tensor (Business KB 는 포함, PD 는 제외) | C | [rterm], [PD-remote] |
| C-19 | TeamViewer VPN | 두 PC 를 1:1 가상 LAN 으로 묶는다. driver 설치에 관리자 권한이 필요하고 Windows 전용이다. 15.64.3 에 legacy 기기용 VPN 이 추가돼 아직 살아 있다 | Bus/Prem/Corp/Tensor | D (가상 네트워크 driver 가 필요하고 화면 제어와는 다른 문제. 필요하면 WireGuard 같은 전용 도구가 낫다) | [vpn], [d140487] |
| C-20 | Windows Authentication 연결 | Windows 계정으로 인증해 연결하고 UAC prompt 까지 제어한다. 15.78.3 부터 기기 페이지에서 바로 쓸 수 있다 | 모든 Remote 사용자 | C | [uac], [d151090] |
| C-21 | Wake-on-LAN | 같은 네트워크에서 켜져 있는 TeamViewer 기기를 거쳐 깨우거나, 고정 IP/DDNS + port forwarding 으로 깨운다. 유선 연결만 되고 Mac 은 sleep 상태에서만 깨울 수 있다. Host 설치와 계정 할당이 필요하다 | RA/Bus/Prem/Corp/Tensor | C (같은 LAN 에 켜진 중계 기기가 있어야 함) | [wol] |
| C-22 | Proxy 지원 | proxy 를 거쳐 연결한다. CLI 로 `--ProxyIP`, `--ProxyUser`, `--ProxyPassword`를 지정한다 | 모든 tier | C | [PD-remote], [cli] |
| C-23 | Web client (브라우저에서 제어) | 설치 없이 web.teamviewer.com 에서 연결한다. Chrome/Firefox/Opera/Edge (Classic KB 는 Safari 15 이상도 포함). 파일 전송은 로컬에서 원격 방향만 되고, web client 의 clipboard 는 plain text 만 된다 | PD 는 모든 tier, Classic KB 는 RA/Prem/Corp/Tensor (문서끼리 다름) | C (브라우저용 client 스택을 따로 만들어야 함) | [get-started], [web-client], [web-ft] |
| C-24 | 브라우저 screen sharing (고객 쪽 설치 없음) | 고객이 링크만 열면 화면, 창, 브라우저 tab 을 공유한다. 보기 전용이고, 제어가 필요하면 QuickSupport 로 넘어간다 | Bus/Prem/Corp ("not yet available for TeamViewer Tensor") | C | [browser-share] |
| C-25 | 모바일 연결 (PC→mobile, mobile→mobile) | MDS(Mobile Device Support) add-on 이 필요하다. 개인 사용자는 "Connections are limited to five minutes each". Android 는 제어, iOS 는 화면 공유만 된다 | Free "Limited", Bus+ 는 add-on | D (모바일은 현재 대상 밖) | [PD-remote], [mobile-mobile] |
| C-26 | Meeting / presentation | 회의와 발표. "The meeting feature is currently in final sunset planning." | Bus/Prem/Corp/Tensor | D (TeamViewer 가 스스로 접는 중) | [meeting] |
| C-27 | QuickJoin | 2024-12-20 에 다운로드 페이지에서 내려갔다 | 폐지 | D (폐지됨) | [d139418] |
| C-28 | Connect to a contact | 연락처 목록에서 연결을 요청하고 상대가 Accept 하면 시작한다 | license 보유자 | D (계정·연락처 서버가 있어야 함) | [contact] |
| C-29 | SOS button, 웹사이트용 TeamViewer Button | 바탕화면 아이콘을 더블클릭해 지원을 요청하거나, 웹사이트에 버튼을 넣는다 | Bus/Prem/Corp | D (지원 업무용 branding 기능) | [PD-remote], [button] |
| C-30 | 마이크·카메라 forwarding | 로컬 마이크와 카메라를 원격 기기로 넘긴다. 15.80.4 에 추가 | Tensor, ONE | D (가상 장치 driver 가 필요) | [av-fwd], [d151503] |
| C-31 | Conditional Access 전용 router | 고객 전용 router 만 쓰고 공용 master*/router* 는 방화벽에서 막는다. 기본값은 모두 차단이고 규칙으로 허용한다 | Tensor + add-on | D (중앙 정책 서버) | [ca] |
| C-32 | Agentless Access | gateway 를 거쳐 agent 를 설치할 수 없는 장비에 SSH, VNC, RDP, port forwarding 으로 접속한다 | Tensor add-on | D (산업 장비용 별도 제품) | [agentless], [agentless-kb] |
| C-33 | Protocol 버전 관리와 옛 버전 차단 | v11/12 는 2025-12-31 에 인터넷 연결이 끊겼고, v13/14 는 2026-10-31 부터 단계적으로 끊긴다. LAN 연결은 서버를 거치지 않아 계속 된다. Free 사용자는 최신 버전만 쓸 수 있다 | - | A (protocol version 협상을 처음부터 넣어야 나중에 옛 client 를 안전하게 끊을 수 있음) | [d141015], [d143746], [latest-version] |

## 3. 보안

### 3.1 기능 표

| ID | 기능 | 설명 | Tier | 분류 | 출처 |
|---|---|---|---|---|---|
| S-01 | 세션 암호화 | 15.73 이상은 "mutually authenticated TLS 1.3" 으로 인증하고, "modern Diffie-Hellman" 으로 PFS 를 얻은 뒤 AES-256-GCM 으로 암호화한다. 양쪽은 master cluster 가 발급한 인증서로 서로를 확인한다. 15.73 미만(legacy)은 RSA-4096 key exchange + AES-256 이다. 15.76.3 부터 TLS 1.3 만 허용하는 policy 가 있다 | 모든 tier | A | [sec-statement], [sec-pdf], [d143411] |
| S-02 | E2E: relay 가 내용을 못 읽음 | "private keys never leave the client device… neither TeamViewer routing servers nor any intermediary systems can decrypt the end-to-end encrypted session traffic" | 모든 tier | A | [sec-statement], [sec-pdf] |
| S-03 | 기기 ID 검증 | TeamViewer ID 는 hardware/software 특성에서 만들고, 서버가 유효한지 검사한다 | 모든 tier | A (우리 쪽에서는 기기 ID + 기기별 key pair 로 대신함) | [sec-statement] |
| S-04 | SRP password 인증 | "Using the Secure Remote Password (SRP) protocol version 6, no password equivalent data is shared… Only a password verifier is stored on the local computer." | 모든 tier | A (SRP 나 다른 PAKE(password 로 key 를 합의하되 password 는 노출하지 않는 protocol)를 써서 signaling/relay 서버에 password 가 드러나지 않게) | [sec-statement] |
| S-05 | Brute-force 방어 | "exponentially increases the latency between connection attempts. It thus takes as many as 17 hours for 24 attempts." 맞는 password 를 넣어야만 초기화된다. 한 ID 를 여러 컴퓨터(botnet)에서 공격하는 경우도 막는다 | 모든 tier | A | [brute-force] |
| S-06 | Attended 동의 prompt | session code 나 QuickSupport 로 들어오면 고객이 "Allow access"를 눌러야 한다. attended 연결에서 이 승인을 policy 로 강제할 수 있다 | 모든 Remote 사용자 | A | [attended], [sec-pdf] |
| S-07 | Stealth mode 없음, 세션 표시 | "There is no function that enables you to have TeamViewer running undetected in the background." attended 세션 중에는 화면 둘레에 주황색 테두리(Screen sharing indicator)가 뜬다 | 모든 tier / policy | A | [sec-statement], [policy-settings] |
| S-08 | Incoming 연결 로컬 log | Host 가 `Connections_incoming.txt`에 기록한다. policy 이름은 "Log incoming connections" | policy | A (값싸고 사고 조사에 꼭 필요) | [log-incoming], [policy-settings] |
| S-09 | TeamViewer fingerprint | 기기 public key 에서 만든 문자열이다. Connection Info 에서 확인하고 전화 등 다른 경로로 대조해 중간자 공격(MITM)을 찾아낸다 | 문서에 없음 | B | [fingerprint] |
| S-10 | Block / Allowlist | 목록의 ID·계정을 막거나(block), 목록에 있는 ID·계정·회사만 허용한다(allow). meeting 에도 적용하는 옵션이 있다 | 회사 단위 목록은 Prem/Corp | B | [allowlist] |
| S-11 | Inactive 세션 자동 종료 | 정해진 시간 동안 입력이 없으면 outgoing 세션을 끝낸다. policy 로 강제할 수 있다 | Windows, 모든 license | B | [timeout] |
| S-12 | 최대 세션 시간 48시간 | "All TeamViewer sessions are limited to a maximum duration of 48 hours… This limit cannot be changed." (TeamViewer 공식 블로그) | 모든 tier | B | [48h] |
| S-13 | 설정 보호 | 설정 변경에 관리자 권한을 요구하거나, 옵션을 password 로 잠그거나, TeamViewer 를 끄지 못하게 한다 | policy / 로컬 설정 | B (unattended host 를 일반 사용자가 풀지 못하게) | [protect-options], [policy-settings] |
| S-14 | Code signing, 자체 무결성 검사 | DigiCert 로 서명한다. 시작할 때 자기 서명을 확인해 "fails to run if inconsistencies are found" | - | B (배포 신뢰도와 변조 방지) | [sec-statement], [sec-pdf] |
| S-15 | 연결 2FA (push 승인) | 이 기기로 들어오는 연결마다 등록된 휴대폰에서 push 로 승인한다. Host 는 Windows 15.17+, mac/Linux 15.22+, 승인 앱은 Android 6.0+ / iOS | 문서에 없음 | C (모바일 앱과 push 서버가 필요. host 화면 승인(S-06)으로 비슷한 효과를 낼 수 있음) | [2fa-conn] |
| S-16 | 계정 2FA (TOTP), 회사 단위 강제 | 계정 login 에 인증 앱의 TOTP 와 recovery code 를 쓴다. 관리자가 회사 전원에게 강제할 수 있다 | 모든 tier | D (계정 시스템이 없으면 필요 없음) | [2fa-account], [2fa-enforce] |
| S-17 | Trusted devices | 새 기기나 브라우저에서 처음 login 하면 이메일 링크(24시간 유효)나 앱 push 로 승인한다 | 모든 Classic 사용자 | D (계정이 있어야 함) | [trusted-devices], [trust-push] |
| S-18 | 계정 데이터 암호화, zero-knowledge 복구 | 계정에 저장한 password·key·chat 을 사용자 password 에서 만든 root key 로 암호화한다. password 재설정에는 64자 recovery code 가 필요하고 "TeamViewer has no access to it" | 모든 tier | D (계정이 있어야 함) | [sec-pdf], [zk-recovery] |
| S-19 | Settings policy 강제 | 관리자가 설정을 배포하고, "Enforced" 로 둔 설정은 로컬에서 바꿀 수 없다. 예: TLS 1.3 강제(15.76.3), personal password 금지(15.65.4), incoming 자동 녹화(15.70.3) | policy 수: Bus 5 / Prem 10 / Corp 15 / Tensor 60 | D (중앙 관리 서버가 있어야 함) | [policy-settings], [policies], [d143411], [d140812], [d142196] |
| S-20 | Connection reports | incoming/outgoing 연결 보고서를 filter 하고 CSV(UTC 시각)로 내보낸다 | Prem/Corp/Tensor | D (중앙 서버) | [conn-reports] |
| S-21 | Event log (audit), SIEM 연동 | 세션과 관리 작업을 기록하고 CSV 와 REST API 로 제공한다. Frankfurt 에 1년 보관한다. Splunk 연동 문서가 있다 | Tensor | D | [event-log], [splunk], [security-explained] |
| S-22 | Security Center | 2FA/SSO 사용 비율, 90일간 안 쓴 사용자 등을 보여주는 보안 점수 화면 | Prem/Corp/Tensor | D | [security-center] |
| S-23 | Conditional Access 승인 강제 | 지정한 계정이 30분 안에 web 이나 mobile push 로 연결을 승인한다. 첫 결정이 최종이다 | Tensor + CA add-on | D | [ca-approval] |
| S-24 | BYOC (Bring Your Own Certificate) | 고객 CA 인증서로 기기를 추가 인증한다. TeamViewer 인증서와 "always in addition to" 관계다 | Tensor | D | [byoc] |
| S-25 | SSO (SAML 2.0), SCIM | email domain 단위 SSO, Entra ID/Okta 사용자 동기화 | Tensor Basic+, ONE Adv+ | D | [sso], [scim] |
| S-26 | Security key redirection | 로컬 FIDO key 나 smart card 를 원격 PC 로 넘긴다. driver 방식이고 Windows 10 64-bit 이상 | Tensor | D (driver 필요) | [sec-key] |
| S-27 | 모바일 앱 생체 잠금 | 앱이 background 로 가면 잠그고 Touch ID/Face ID/지문으로 연다 | - | D (모바일) | [biometric] |
| S-28 | 인증·데이터센터 | ISO 27001, SOC2 Type2, HIPAA, TISAX 등. 민감 데이터를 담는 서버는 독일과 오스트리아에 있다 | - | D (회사 단위 인증) | [trust-center], [sec-pdf] |
| S-29 | 사기(scam) 의심 연결 경고 | "potential fraudulent background" 연결에 경고를 띄운다는 문장이 검색 요약에만 있고, 현재 페이지(2024-10-01 수정)에는 없다. 미확인 | - | C | [scam] |

### 3.2 설계에 바로 쓰이는 원문

- 암호: "Starting with TeamViewer version 15.73 and later, session authentication and attestation leverage mutually authenticated TLS 1.3, which introduces Perfect Forward Secrecy (PFS) through modern Diffie-Hellman key exchange mechanisms. After the secure handshake, session data continues to be protected using AES-256-GCM encryption." ([sec-pdf] p.19, [sec-statement] 2026-05-29 수정)
- 문서는 ECDH 라는 단어를 쓰지 않고 "modern Diffie-Hellman"이라고만 한다. cipher suite 목록은 "under mutual non-disclosure agreements (NDA)"로만 제공하고, backend 통신 일부는 아직 자체 protocol 이다 ([sec-pdf]).
- 2017년 보안 문서는 RSA 2048 + AES 256 으로 적혀 있다 ([sec-pdf-2017]). 현재 legacy protocol 의 값은 RSA-4096 이다.
- relay 역할: 2017 문서는 "not even we, as the operators of the routing servers, can read the encrypted data traffic"라고 쓴다 ([sec-pdf-2017]). 이 구조에서는 master 가 각 peer 의 public key 를 전달하고, peer 끼리 대칭 key 를 교환한다.
- random password 갱신: "TeamViewer generates a new password after it restarts, after the session (recommended and the new default setting), or when manually requested." ([sec-pdf] p.15)
- 최근 보안 공지 ([bulletins]). 네 건 모두 암호가 아니라 동의·승인·입력 검증 단계에서 났다.
  - TV-2026-1003: "Allow after confirmation" 설정을 확인 전에 우회. 15.74.5 에서 수정.
  - TV-2026-1007: macOS 15.80 미만에서 unattended 경로로 연결 2FA 승인을 우회.
  - TV-2026-1008: 상대가 보낸 파일 경로를 검증하지 않음.
  - TV-2026-1009: Linux 에서 chat 링크 처리로 command injection.

## 4. 세션 중 기능

| ID | 기능 | 설명 | Tier | 분류 | 출처 |
|---|---|---|---|---|---|
| F-01 | Clipboard 자동 동기화 (text) | 양방향 자동 동기화다. "The sync only works if clipboard synchronization is enabled on both devices." 15.73.3 부터 supporter 쪽 세션 창이 focus 일 때만 동기화한다. 15.70.3 부터 end user 가 세션 중 자기 panel 에서 켜고 끌 수 있다 | 모든 사용자 | A | [clipboard], [d142871], [d142196] |
| F-02 | Clipboard 자동 동기화 (image) | text 와 같은 경로로 image 도 동기화한다 | 모든 사용자 | B | [clipboard] |
| F-03 | One-time send to / from remote | 마지막으로 복사한 항목 하나만 한 방향으로 보낸다 | 모든 사용자 | B | [clipboard] |
| F-04 | Paste as keystrokes | clipboard 내용을 키 입력으로 쳐 넣는다. 붙여넣기가 막힌 login 칸이나 UAC 칸에 쓴다. 이렇게 넣은 내용은 clipboard 동기화로 공유되지 않는다 | 모든 사용자 | B | [paste-keys], [clipboard] |
| F-05 | 파일 clipboard (파일 복사·붙여넣기) | TeamViewer 문서에는 text 와 image 만 나오고 파일 clipboard 는 없다. 파일은 file transfer 나 drag & drop 으로 보낸다 | - | C | [clipboard] |
| F-06 | File transfer 창 | 왼쪽 로컬, 오른쪽 원격 2단 창이다. Send/Receive, 작업 queue, 창 안 drag & drop 을 지원한다. 원격 Mac 은 Full Disk Access 권한이 필요하다 | Free 는 한 번에 1개, queue 는 RA+ | B | [toolbar], [PD-remote], [mac] |
| F-07 | 원격 창 ↔ 로컬 바탕화면 drag & drop | 원격 화면 창과 로컬 바탕화면 사이에서 파일을 끌어다 놓는다. Windows 만 되고, policy "Disable remote drag & drop integration"으로 끌 수 있다 | - | C | [transfer-files], [policy-settings] |
| F-08 | File box | 세션 중 상대와 파일을 공유하는 panel | KB 는 Bus+, toolbar 는 Free (문서끼리 다름) | C | [file-box], [toolbar] |
| F-09 | 연결 없이 파일 보내기 | Windows 우클릭 "Send to → TeamViewer"나 연락처 목록으로 보낸다. 받는 쪽이 확인한다 | - | D (연락처 서버가 있어야 함) | [transfer-files] |
| F-10 | TeamViewer Printing | 원격 PC 에서 로컬 프린터로 인쇄한다. 원격에 print driver 를 설치한다. Windows 와 macOS 사이만 되고, Windows ARM native 판에서는 빠진다 | RA/Bus/Prem/Corp/Tensor | C (가상 프린터 driver 필요) | [printing], [arm] |
| F-11 | Computer sound | 원격 PC 소리를 로컬 스피커로 보낸다 | toolbar 는 Free, Business 기능 목록에는 "Remote Sound" | B | [toolbar], [business-kb] |
| F-12 | Chat (기록 저장 포함) | 세션 중 채팅. 기록을 text 파일로 저장한다. chat 은 E2E 로 암호화된다 | Free 는 세션 중 쓸 수 없음 | B | [toolbar], [chat-history], [sec-statement] |
| F-13 | Audio / Video call | VoIP 와 webcam 통화 | Free 는 세션 중 쓸 수 없음 | C | [toolbar] |
| F-14 | Whiteboard / annotation | 원격 화면 위에 그린다. Pen, Highlighter, Eraser, Rectangle, Ellipse, Text, Speech balloon, Save, Clear. 양쪽 모두 그릴 수 있다 | Free 포함 | C | [whiteboard], [toolbar] |
| F-15 | 세션 녹화 (수동), 재생·변환 | Start / Pause / Stop and save. 독자 형식 .tvs 로 화면, 원격 소리, webcam, VoIP 를 녹화한다. 변환은 Windows 에서만 되고 OS 에 있는 codec 을 쓴다 (v14 는 AVI 라고 적음). MP4 직접 변환은 못 찾음 | toolbar·Free KB 는 Free, PD 는 Bus+ (문서끼리 다름) | C | [record], [toolbar], [v14-manual] |
| F-16 | 자동 녹화 policy, SFTP 업로드 | incoming/outgoing 세션을 자동으로 녹화하고, 멈추거나 일시정지하지 못하게 하며, 저장 폴더를 지정한다. Tensor 는 SFTP 업로드도 된다 | policy / Tensor | D (중앙 policy 가 있어야 함) | [record-policy], [record-sftp] |
| F-17 | Screenshot | 원격 화면을 파일로 저장하거나 clipboard 로 복사한다 | Free | B | [toolbar] |
| F-18 | Show black screen (privacy) | 원격 화면을 보안 이미지로 가리고 원격 입력도 막는다. Windows 7 이하는 monitor driver 가 필요하다. macOS 는 원격이 15.8 이상이어야 하고 Accessibility 권한이 필요하며, system password 칸의 키 입력은 막지 못한다. 원격 사용자는 Ctrl+Alt+Del 이나 Cmd+Option+Esc 로 풀 수 있다 | RA/Bus/Prem/Corp/Tensor | C (화면은 꺼 두면서 capture 는 계속해야 해서 OS 마다 어려움) | [black-screen] |
| F-19 | Lock | Lock now / Lock on session end / Sign out on remote computer. 세션을 끝낼 때 잠금은 Always / Never / Automatic 중에서 고른다. Automatic 은 시작 때 잠겨 있었으면 끝날 때 다시 잠근다 | Free | B | [toolbar], [v14-manual] |
| F-20 | Remote reboot + 재연결 | Reboot 와 Reboot in safe mode(네트워크 포함) 중에서 고르고, 재시작 후 다시 연결하는 창이 뜬다. macOS 에서는 "not yet supported" | Free | C (safe mode 에서도 도는 service 가 필요) | [toolbar], [v14-manual] |
| F-21 | Information | Remote System Information, Connection Information(시간, 해상도, 버전, fingerprint), Session Information | Free | B | [toolbar], [fingerprint] |
| F-22 | Dashboard | 프로세스, 성능, 디스크 상태, 보안, 시스템 환경 정보. 프로세스 항목에서 원격 Task Manager 를 연다 (v14) | Free | C | [toolbar], [v14-manual] |
| F-23 | Remote update | 원격 TeamViewer 를 최신 버전으로 올리고, 끝나면 다시 연결한다 | Free | C | [toolbar], [v14-manual] |
| F-24 | Leave note / Comment | 세션이 끝나도 고객 화면에 남는 메모, 그리고 연결별 메모(connection report 에 기록됨) | Free | C | [toolbar] |
| F-25 | Scripts | Windows 는 .bat/.cmd/.ps1, macOS/Linux 는 .sh 를 실행한다. 원격 사용자가 요청마다 수락해야 한다 | Bus 5개 / Prem 15개 / Corp 30개 / Tensor 50개 | C | [scripts], [PD-remote] |
| F-26 | Quick Steps | 자주 쓰는 원격 동작을 한 번에 실행한다 | Prem+ | C | [toolbar] |
| F-27 | Session Insights / Tia (AI) | AI 세션 요약, 실시간 시스템 데이터로 원인 추정, 승인을 거친 자동 조치(2026-09 GA). AI credit 으로 과금한다 | Bus+ (credit 방식) | D (AI 서비스와 과금 구조가 있어야 함) | [session-insights], [tia], [tia-press] |
| F-28 | Augment session (AR) | 상대 쪽 카메라 화면에 AR 로 표시하며 지원한다 | - | D (별도 AR 제품군) | [toolbar] |

## 5. 제어권 관리

| ID | 기능 | 설명 | Tier | 분류 | 출처 |
|---|---|---|---|---|---|
| CT-01 | Access control 기본 단계 (Full access / View and show / Deny) | 들어오는 연결의 권한 수준을 정한다. Full access 는 확인 없이 제어한다. View and show 는 화면 보기와 포인터 표시만 되고 제어는 안 된다. Deny 로 두면 연결 요청 창도 뜨지 않는다 | 모든 tier | A | [policy-settings], [restrict-access], [v14-manual] |
| CT-02 | Confirm all | 모든 기능이 열려 있지만 "the remote computer's user must confirm access before the feature can be used". 2026년에 이 확인을 우회하는 취약점(TV-2026-1003)이 있었다 | 모든 tier | B | [policy-settings], [bulletins] |
| CT-03 | Custom settings (기능별 권한) | 화면 보기, 제어, 파일 전송, VPN, 원격 입력 잠금, 원격 TeamViewer 제어, File box, 인쇄, script 실행을 하나씩 Allowed / Upon confirmation / Denied 로 정한다 (v14) | 모든 tier | C | [v14-manual], [policy-settings] |
| CT-04 | Outgoing access control, 충돌 규칙 | 나가는 연결에도 같은 수준을 건다. 내 outgoing 설정과 상대 incoming 설정이 다르면 더 제한적인 쪽을 적용하고, 차이를 대화상자로 보여준다 (v14). QuickSupport 에는 access 설정이 없다 | 모든 tier | C | [policy-settings], [v14-manual] |
| CT-05 | Disable remote input | 원격 PC 의 키보드와 마우스를 막는다. 원격 사용자가 Ctrl+Alt+Del 을 누르면 풀린다 (v14) | Free | B | [toolbar], [v14-manual] |
| CT-06 | Host 쪽 세션 즉시 종료 hotkey | incoming 세션을 Ctrl+Alt+F8 로 끊는다. 2026-09 부터 Settings → Remote control → Session settings → Incoming connections → "Hotkey to end incoming sessions"에서 바꿀 수 있고, "Set to default"로 되돌린다 | 문서에 없음 | A (host 사용자가 언제든 제어를 되찾는 안전장치) | [d151662] |
| CT-07 | 원격 사용자가 black screen / 입력 차단 해제 | Ctrl+Alt+Del (Windows/Linux), Cmd+Option+Esc (macOS) | 기능 tier 를 따름 | B | [black-screen] |
| CT-08 | Switch sides with partner | 역할을 뒤집어 상대가 내 PC 를 보고 제어한다 | toolbar 는 "Requires license", Free KB 는 포함, PD 는 Bus+ (문서끼리 다름) | B | [toolbar], [PD-remote], [free-kb] |
| CT-09 | Invite more participants / session handover | 등록된 연락처를 세션에 초대하고, 초대받은 사람은 "must be confirmed within 20 seconds". 원래 supporter 가 나가면 handover 가 된다. 참가자는 모두 같은 권한으로 제어한다 (v14) | toolbar 는 Free, PD 는 Prem (문서끼리 다름) | C | [toolbar], [PD-remote], [v14-manual] |
| CT-10 | Meeting 제어 넘기기 | "Assign as presenter", "Allow control". 한 번에 한 명만 제어한다 | Bus+ (Meeting 은 종료 예정) | D (Meeting 제품이 정리되는 중) | [meeting-control], [meeting] |
| CT-11 | Login 화면 연결 시 full access | Windows login 화면으로 연결하면 자동으로 full access 를 준다 (v14). macOS 에는 "Full access control when a partner is connecting to the login window" 설정이 있다 (15.79.4 에서 버그 수정) | - | C | [v14-manual], [d151306] |
| CT-12 | Script 실행 권한 | 기본은 요청마다 원격 사용자가 수락한다. Access control 이나 policy 로 항상 거부하거나, 무인 기기에서는 요청 없이 허용할 수 있다 | Bus+ | C | [scripts] |

못 찾은 것: 1:1 원격 제어 세션에서 원격 쪽 사용자가 제어권을 "요청"하는 기능은 1차 출처에 없다. TeamViewer 는 "Switch sides"(역할 뒤집기)와 "Disable remote input"(상대 입력 막기)으로 제어권을 다룬다.

## 6. 키보드·단축키

| ID | 기능 | 설명 | Tier | 분류 | 출처 |
|---|---|---|---|---|---|
| K-01 | Send key combinations | Win+E, Ctrl+P, Alt+Tab 같은 조합을 로컬이 아니라 원격에서 실행한다. toolbar 에서 켜고 끄며, 기본값은 설정과 policy 로 정한다. macOS 에서는 "Send system key combinations"라고 부른다 | Free | A | [toolbar], [key-commands], [policy-settings] |
| K-02 | Send Ctrl+Alt+Del | toolbar 버튼으로 보낸다. 대상은 Windows 만이다 | Free | B (Windows 쪽 구현 방식은 host-platform 조사 문서 참고) | [toolbar], [toolbar-classic-mac] |
| K-03 | 키보드 layout 변환 (기본 동작) | 기본은 layout 을 변환해서 보낸다. Direct keyboard mode 설명이 "without translating keyboard layouts"라고 대비해서 쓰는 데서 알 수 있다. "Use local keyboard layout"이라는 옵션 이름은 1차 출처에서 못 찾음 | Free | A (scancode 로 보낼지 문자로 보낼지는 처음에 정해야 하는 설계 결정) | [direct-kbd] |
| K-04 | Direct keyboard mode | layout 변환 없이 누른 키를 그대로 원격에 보낸다. Windows → Windows 만 된다. 15.61.3 에서 세션 중에 켤 수 있게 됐다 | Free | B | [direct-kbd], [d139571] |
| K-05 | Windows ↔ Mac 키 대응 | Windows key ↔ Command, Ctrl ↔ Control, Alt ↔ Option, Backspace ↔ Delete | - | B (macOS 지원 때 필요) | [key-commands] |
| K-06 | macOS toolbar 사용자 지정 | toolbar 를 우클릭해 항목과 순서를 바꾼다. macOS 만 된다 | Free | C | [toolbar] |
| K-07 | Android touch / mouse mode | 폰에서 PC 를 조작하면 mouse mode 만 되고, 6.5인치 이상 tablet 은 둘 다 된다 | - | D (모바일) | [android-touch] |

참고:
- clipboard 를 키 입력으로 보내는 "Paste as keystrokes"는 F-04 에 있다.
- host 쪽 세션 종료 hotkey 는 CT-06 에 있다.
- TeamViewer 자체 단축키를 사용자가 바꾸는 기능은 CT-06 말고는 1차 출처에서 못 찾았다. 문서에 나오는 단축키는 file transfer 창의 F5, DEL, Ctrl+Shift+N 정도다 (v14).

## 7. 화면: 품질, scaling, 해상도, 멀티 모니터

| ID | 기능 | 설명 | Tier | 분류 | 출처 |
|---|---|---|---|---|---|
| V-01 | Quality: Auto select | 연결 상태에 맞춰 품질을 자동으로 고른다 (기본값) | Free | A | [toolbar] |
| V-02 | Quality preset, Custom | Optimize speed / Optimize quality / Custom. Custom 에는 Colors, Quality slider(왼쪽이 lossy 압축), Fast video streaming, GUI animations, application compatibility 가 있다 | Free | B | [toolbar], [v14-manual] |
| V-03 | Color depth | Custom 의 "Colors" 항목. Connection Info 에 원격 해상도와 color depth 가 나온다 (v14) | Free | C (영상 codec 방식이면 의미가 작음) | [v14-manual] |
| V-04 | Scaling | Best fit / Original / Scaled 에 전체 화면 전환이 따로 있다. Classic macOS 는 Original / Scaled 만 있다 | Free | A | [toolbar], [toolbar-classic-mac] |
| V-05 | Screen Resolution (원격 해상도 변경) | "Change the current screen resolution of the remote device" | Free | B | [toolbar] |
| V-06 | Hide wallpaper | 원격 배경화면을 검은색으로 바꿔 전송량을 줄인다 | Free | B | [toolbar] |
| V-07 | Show remote cursor | 원격 컴퓨터의 마우스 포인터를 보여준다 | Free | A | [toolbar] |
| V-08 | Refresh screen | 화면이 멈춘 것 같을 때 새 화면을 요청한다 | Free | B | [toolbar] |
| V-09 | Select single window / whole desktop | 창 하나만 보거나 데스크톱 전체를 본다 | Free | C | [toolbar] |
| V-10 | 모니터 전환 | 원격 모니터를 번호로 골라 전환한다 | Free | A (전환이 없으면 보조 모니터에 아예 갈 수 없음) | [toolbar], [multi-monitor], [PD-remote] |
| V-11 | Show all monitors | 모든 원격 모니터를 한 창에 보여준다 | Free | B | [multi-monitor] |
| V-12 | 모니터마다 창 / tab 따로 | "Monitors as individual windows"로 모니터마다 창이나 tab 을 따로 띄운다. 로컬 모니터 여러 대에 나눠 보려면 같은 기기에 여러 번 연결하며, 이때 channel 은 1개만 쓴다 | Free | C | [toolbar], [multi-monitor] |
| V-13 | Virtual monitor | 세션 중 원격에 가상 모니터를 추가한다. Windows → Windows 만 되고, 원격은 Windows 10 2004 이상에 Easy Access 가 필요하다. 모니터 없는(headless) Windows 는 자동으로 가상 모니터를 쓴다 | RA/Prem/Corp/Tensor | C (가상 display driver 필요) | [virtual-monitor], [black-screen-trouble] |

참고:
- "Stream in 4K" 같은 옵션 이름은 없다. 기능 페이지는 "Full resolution and support for multi-monitor-setups… intelligent scaling" 정도만 말한다 ([4k]).
- 모니터 없는 서버에 4K dummy plug 를 꽂으라는 안내는 2차 출처에만 있다. 미확인(2차 출처): [helpwire].
- 사용자가 frame rate 를 직접 고르는 옵션은 1차 출처에서 못 찾았다. 관련 옵션은 Custom 의 "Fast video streaming"뿐이다.
- CPU 사용률이 높을 때는 원격(Windows 8 이하)의 GPU hardware acceleration 을 끄라는 troubleshooting 안내가 있다 ([high-cpu]).
- Linux Wayland(GNOME)에서는 "Only single-monitor setups"만 된다 ([wayland]).

## 8. 관리·엔터프라이즈

| ID | 기능 | 설명 | Tier | 분류 | 출처 |
|---|---|---|---|---|---|
| M-01 | 개인 무료 사용, 상업 사용 감지 | 본인·가족·친구의 기기만 무료다. 상업 사용이 의심되면 연결이 timeout 되고 reset 을 신청해야 한다. 감지 방식은 공개하지 않는다 | Free | D (라이선스 정책) | [personal-use], [commercial-suspected] |
| M-02 | 기기 관리 (group, 공유, 권한 수준) | 회사 단위 managed device. 한 기기를 여러 group 에 넣고 권한을 상속한다. 공유 권한은 Read-only / Read/Write / Full control | group 이 있는 tier | D (중앙 서버와 계정이 있어야 함) | [device-mgmt], [d143237], [d142442] |
| M-03 | Device custom fields | 기기마다 자유 입력 필드를 둔다 | Prem 5 / Corp 15 / Tensor 25 | D | [custom-fields] |
| M-04 | Admin settings / Management Console | policy, 기기, 사용자, role, 보고서를 client 와 web 에서 관리한다. 독립 Management Console 은 2026년 말에 종료된다 | Prem/Corp/Tensor 관리자 | D | [admin-settings], [d151689] |
| M-05 | User roles | custom role 을 최대 100개 만들고, 사용자의 권한은 받은 role 들의 합이다 | Prem+ | D | [roles] |
| M-06 | Multitenancy | 부모·자식 회사 구조와 license 배분 | Tensor, ONE | D | [multitenancy] |
| M-07 | MSI 설치와 설치 parameter | Host/Full MSI. `CUSTOMCONFIGID`, `ASSIGNMENTID`, `SETTINGSFILE`(.tvopt), `INSTALLVPN` 등 | Corp, Tensor | C (MSI 와 무인 설치 옵션 자체는 Windows 배포에 쓸모 있음. 계정 할당 부분은 M-08) | [msi] |
| M-08 | 계정 할당 CLI, 할당 링크, rollout 설정 | `TeamViewer.exe assignment --id <ID>`. 15.67.3 부터 설치와 할당을 한 번에 하는 링크가 있다 | Corp/Tensor | D (계정 서버가 있어야 함) | [msi], [d141877], [d141320] |
| M-09 | Intune / GPO / SCCM / Jamf 배포 가이드 | 기업 배포 도구별 가이드 | Corp/Tensor | D | [deploy-guide] |
| M-10 | Custom QuickSupport / Custom Host (branding) | logo, 문구, 색, SOS 버튼, 기본 담당자, 만족도 설문을 넣는다. 15.51.5 부터 모바일 custom QuickSupport 도 된다 | Bus+ | D (지원 업무용 branding) | [qs], [host], [d134409] |
| M-11 | Web API | REST + OAuth2 로 사용자, group, 세션, 보고서를 다룬다. 호출 한도는 Bus 7,200 ~ Tensor 48,000 회/24h | Bus+ | D | [api], [PD-remote] |
| M-12 | Integrations | ServiceNow, Salesforce, Microsoft Teams, Jira, Zendesk, Intune, Jamf, Slack 등 | Corp (Standard 묶음), Tensor/ONE (Enterprise 묶음) | D | [integrations], [intune-press] |
| M-13 | Windows LAPS 연동 | Entra/Intune 의 로컬 관리자 password 를 세션 중 키 입력으로 넣는다. password 는 TeamViewer backend 를 거치지 않는다 | Tensor | D | [laps], [sec-pdf] |
| M-14 | Service Queue / Service Desk | session code 기반 지원 요청 관리(24시간 뒤 Expired), 이메일을 ticket 으로 받는 Service Desk | Bus+ / Prem+ | D (지원 업무 도구) | [service-queue], [service-desk] |
| M-15 | 고객 만족도 평가 | 세션이 끝나면 평가 양식을 보여준다 | Prem+ | D | [qs-classic] |
| M-16 | Remote Management add-on | Monitoring, Asset, Patch, Endpoint protection, MDM, Backup | add-on | D | [PD-rm] |
| M-17 | TeamViewer DEX | 1E 인수(2024-12 발표)로 얻은 사용자 경험 분석과 자동화 | DEX license | D | [dex-press], [PD-dex] |
| M-18 | TeamViewer ONE | Remote + AI + RMM + DEX 를 묶은 license | ONE | D | [press-one], [PD-one] |
| M-19 | Frontline / Assist AR / Pilot | AR 과 smart glass 를 쓰는 현장 지원. Pilot 은 갱신만 받는다 | 별도 제품 | D | [frontline], [PD-ar], [PD-pilot] |
| M-20 | IoT / Embedded | 장비 모니터링과 원격 제어 | IoT license | D | [PD-iot], [embedded] |
| M-21 | TeamViewer Intelligence (AI 묶음) | Session Insights, Tia, Tia Troubleshooting(승인 기반 조치) | AI credit (Bus 25 ~ Tensor 150/월) | D | [intelligence-press], [tia-press], [d151600] |
| M-22 | Command line parameters | `-i`, `--Password`, `-m fileTransfer`, `--ac`, proxy 설정으로 연결을 script 에서 시작한다 | 모든 tier | C | [cli] |

## 9. 플랫폼

### 9.1 플랫폼별 지원

| ID | 플랫폼 | 지원 버전과 형태 | 분류 (이 프로젝트 기준) | 출처 |
|---|---|---|---|---|
| P-01 | Windows (host·controller) | Windows 11 23H2~25H2, Windows 10 22H2. 완전한 제어가 되고 Full client, Host, QuickSupport 가 있다. Windows 7/8.1 과 Server 2008R2/2012 는 15.64 가 마지막 버전 | A | [supported-os], [old-windows] |
| P-02 | Windows UAC / secure desktop | Host 와 Full client 는 service 로 돌아 UAC 를 제어한다. QuickSupport 는 권한 상승 없이 도므로 Windows Authentication 으로 로그인해야 UAC 를 제어할 수 있다 | B | [uac] |
| P-03 | Windows login 화면, unattended | Host 설치와 "Start TeamViewer with Windows" 설정이 필요하다 | B | [uac], [unattended] |
| P-04 | Windows Server / RDP 다중 사용자 | RDP session 마다 TeamViewer ID 가 하나씩 생기고, console 용 "Server ID"가 따로 있다. QuickSupport 는 service 가 아니라서 안 된다. RDP 창을 최소화하면 검은 화면이 된다 | D (서버 다중 세션은 범위 밖) | [win-server] |
| P-05 | Windows on ARM | 15.80 부터 native. "All capabilities… with the exception of Windows driver-based features like VPN, Remote Printer, and SmartCard support" | C | [arm], [d151503] |
| P-06 | macOS (host·controller) | macOS 13 ~ 27 지원. 권한 네 가지가 필요하다 (9.3) | C (다음 단계. 권한 흐름은 미리 설계) | [supported-os], [mac] |
| P-07 | Linux | Classic UI 만 있다. X11 은 완전 지원, Wayland 는 "still experimental"이다 (GNOME/KDE 는 attended 만, GNOME 은 단일 모니터) | D (현재 대상 밖) | [supported-os], [wayland] |
| P-08 | iOS / iPadOS | 17 이상. Broadcast Extension 으로 화면 공유만 된다. "iOS does not allow the remote controlling of any iOS device." | D (모바일) | [supported-os], [ios-share] |
| P-09 | Android | 8 이상. 제어하려면 제조사 add-on 이나 Universal Add-On(Accessibility service)이 필요하고, 없으면 화면 공유만 된다 | D (모바일) | [supported-os], [android-mfr], [android-universal] |
| P-10 | ChromeOS | Android 앱으로 화면 공유만 된다. "Full remote control of Chrome OS is officially not supported yet." | D | [supported-os], [mobile-mobile] |
| P-11 | Raspberry Pi / Embedded | Raspberry Pi OS 용 arm64/armv7 client, 별도 Embedded agent | D | [supported-os], [embedded] |

### 9.2 플랫폼별 기능 차이

| 기능 | Windows | macOS | Linux | 출처 |
|---|---|---|---|---|
| TeamViewer VPN | 됨 (driver) | 안 됨 | 안 됨 | [vpn] |
| 원격 인쇄 | 됨 (ARM native 제외) | 됨 | 언급 없음 | [printing], [arm] |
| Black screen | 됨 (Windows 7 이하는 driver 필요) | 15.8+ 와 Accessibility 권한 필요, system password 칸 입력은 못 막음 | 됨 | [black-screen] |
| 가상 모니터 | Windows → Windows, 원격 Win10 2004+ | 안 됨 | 안 됨 | [virtual-monitor] |
| Remote Terminal | Win10 1809+ | 안 됨 | 안 됨 | [rterm] |
| Direct keyboard mode | Windows → Windows 만 | 안 됨 | 안 됨 | [direct-kbd] |
| Reboot / safe mode | 됨 | "not yet supported on macOS" | - | [toolbar] |
| Send Ctrl+Alt+Del | 대상이 Windows 일 때만 | - | - | [toolbar-classic-mac] |
| incoming 자동 녹화 | 됨 | 됨 | 안 됨 | [record-policy] |
| Clipboard 동기화 | text + image | text + image | text + image | [clipboard] |
| 멀티 모니터 | 됨 | 됨 | X11 은 됨, Wayland GNOME 은 단일 모니터 | [toolbar], [wayland] |
| 마이크·카메라 forwarding | 됨 (15.80.4) | changelog 에 없음 | 안 됨 | [d151503] |
| Security key redirection | 됨 (ARM native 제외) | 안 됨 | 안 됨 | [sec-key], [arm] |
| Wake-on-LAN 대상 | sleep, 최대 절전, 전원 꺼짐에서 깨움 | sleep 에서만 | Windows 와 같음 | [wol] |

### 9.3 macOS 권한

- 필요한 권한 네 가지 ([mac]):
  - Screen Recording: 화면 보기
  - Accessibility: 마우스와 키보드 제어
  - Full Disk Access: 파일 전송
  - Remote Desktop: "Ensures that unattended access remains functional for TeamViewer full client and Host"
- 권한을 받는 방법 ([mac]):
  - 처음 실행할 때와 권한이 빠졌을 때 "Set up your Mac for TeamViewer" 안내 창이 뜨고, 버튼을 누르면 그 설정 화면이 열린다.
  - Help → Check system access 에서도 확인할 수 있다.
  - 원문: "TeamViewer cannot grant this access by itself, nor can it be granted remotely through a TeamViewer connection."
- 권한이 없을 때 ([mac], [black-screen-trouble]):
  - 상대에게는 "only the TeamViewer app and the desktop background"만 보인다.
  - 마우스·키보드 제어와 파일 전송이 안 된다.
  - Screen Recording 권한이 없는 것이 macOS 에서 검은 화면이 뜨는 가장 흔한 원인이다.
- macOS 버전별로 달라진 점:
  - 10.14 부터 Accessibility 가 필요하고 ([black-screen]), 10.15 에서 Screen Recording 과 파일 접근 권한이 생겼다 ([mac-catalina]).
  - macOS 15 Sequoia 는 매달 화면 녹화 경고가 다시 뜬다. TeamViewer 는 15.58 부터 Remote Desktop consent 로 대응한다. 같은 시기에 Local Network 권한도 생겨서, direct UDP 연결과 기기 탐색에 필요하다 ([mac-sequoia]).
  - macOS 27 에서는 Accessibility 의 이름이 "Device Control and Data Access"로 바뀐다. MDM 으로 관리하는 Mac 은 PostEvent PPPC(MDM 이 앱 권한을 미리 허용하는 설정 profile)를 따로 내려보내야 입력, hotkey, 입력 차단이 동작한다 ([mac-27], [mac-27-mdm]).
- MDM 의 한계 ([mac-27-mdm]):
  - Screen Recording 은 "must still be granted interactively by the user and cannot be approved through MDM".
  - Full Disk Access 는 MDM 으로 내려보낼 수 있다.

## 10. 1차 출처끼리 다른 곳과 못 찾은 항목

### 10.1 TeamViewer 문서끼리 다른 곳

| 항목 | 한쪽 | 다른 쪽 | 이 문서의 선택 |
|---|---|---|---|
| Session recording tier | toolbar·Free KB: Free | PD: Bus+ | PD |
| Switch sides tier | Free KB: Free 포함 | toolbar "Requires license", PD: Bus+ | PD |
| Invite more participants tier | toolbar: Free | PD: Prem ("Invite additional participants and session handover") | PD |
| File box tier | toolbar: Free | File box KB: Bus+ | 둘 다 적음 |
| Remote Terminal tier | Business KB: 포함 | PD: Prem+ | PD |
| Web client tier | PD: 모든 tier | Classic KB: RA/Prem/Corp/Tensor | PD |
| Easy Access tier | Classic KB: 모든 사용자 | 배포 가이드: Corp/Tensor | 둘 다 적음 |
| macOS black screen | Classic macOS toolbar: "Windows only" | Black screen KB: Win/mac/Linux (15.8+) | 최신 KB |
| Android / iOS 최소 버전 | Android Host KB 5.1+, iOS KB 15 | 지원 OS 페이지(2026-08): Android 8+, iOS 17+ | 지원 OS 페이지 |

### 10.2 못 찾았거나 확인하지 못한 항목

- 원격 쪽 사용자의 제어권 요청(request control): 못 찾음.
- "Use local keyboard layout" 옵션, TeamViewer 자체 단축키 사용자 지정(CT-06 제외): 못 찾음.
- 사용자가 고르는 frame rate 옵션, "Stream in 4K" 옵션: 못 찾음.
- 파일 clipboard 동기화: 문서에 없음 (문서에는 text 와 image 만 나옴).
- 녹화 파일을 MP4 로 바로 변환하는 기능: 못 찾음.
- 연결 중 UI 가 direct 인지 relay 인지 보여주는지: 못 찾음. Connection Information 은 시간, 해상도, 버전, fingerprint 만 문서화돼 있다.
- LAN 모드의 포트 번호, session code 형식, session link 유효 기간: 못 찾음. service case 가 24시간 뒤 만료된다는 것만 확인했다.
- Passkey 나 신원 확인 기능: 못 찾음. 계정 2FA 는 TOTP 만 문서화돼 있다.
- 사기 의심 연결 경고(S-29): 검색 요약에만 있고 현재 페이지에는 없음. 미확인.
- 원격이 Mac 일 때 소리 전송: 미확인. 오래된 사용자 포럼 글만 있다.
- 4K dummy plug 안내: 미확인(2차 출처), [helpwire].
- Inactive 세션 timeout 의 선택 가능한 시간 범위: 본문 text 에 없음 (화면 이미지 안에만 있는 것으로 보임).

## 11. 사용자 요청 항목별 정리

### 11.1 제어권 관리

TeamViewer 는 세 층으로 제어권을 다룬다.

1. 연결 전에 정하는 권한 수준.
   - Full access / Confirm all / View and show / Custom / Deny (CT-01 ~ CT-03).
   - Custom 은 기능마다 Allowed / Upon confirmation / Denied 를 고른다.
   - 내 outgoing 설정과 상대 incoming 설정 중 더 제한적인 쪽을 따른다 (CT-04, v14).
2. 세션 중 전환.
   - Disable remote input: host 쪽 입력 막기 (CT-05).
   - Show black screen (F-18).
   - Switch sides: 방향 뒤집기 (CT-08).
   - Invite participants / handover: 제어하는 사람 바꾸기 (CT-09).
   - v14 기준으로는 참가자 모두가 같은 권한으로 동시에 제어한다. 제어권을 "한 명에게만 주는" 방식은 Meeting(CT-10)에만 있다.
3. host 사용자의 비상 탈출.
   - Ctrl+Alt+F8 로 세션을 끊는다. 이 hotkey 는 바꿀 수 있다 (CT-06).
   - Ctrl+Alt+Del / Cmd+Option+Esc 로 입력 차단과 black screen 을 푼다 (CT-07).

MVP 제안:
- CT-01 의 3단계(Full / View only / Deny), host 동의 prompt(S-06), host 쪽 종료 hotkey(CT-06)를 A 로 넣는다.
- 기능별 권한(CT-03)은 파일 전송이나 clipboard 를 넣을 때 같이 설계한다.
- 여러 명이 참여하는 세션을 나중에 넣을 생각이면 두 방식 중 하나를 고르는 결정이 필요하다. 하나는 TeamViewer 처럼 전원 동시 제어이고, 다른 하나는 한 명에게만 제어 토큰을 주는 방식이다. 이 결정이 protocol 모양을 바꾸므로 초기에 정해 두는 편이 낫다.

### 11.2 단축키

- 특수 조합 전달:
  - "Send key combinations" toggle 을 켜면 Win, Alt+Tab 같은 키를 로컬 OS 가 먹지 않고 원격으로 넘긴다 (K-01).
  - Ctrl+Alt+Del 은 별도 버튼으로 보내고, 대상이 Windows 일 때만 된다 (K-02).
- layout:
  - 기본은 layout 을 변환해서 보낸다. Direct keyboard mode(Windows → Windows)는 변환 없이 그대로 보낸다 (K-03, K-04).
  - 한국어 IME 나 한/영 키를 다루려면 scancode 방식과 문자 방식 중 무엇을 기본으로 할지 초기에 정해야 한다.
- 이기종 연결:
  - Windows ↔ Mac 키 대응표가 있다 (K-05).
- 사용자 지정:
  - host 쪽 세션 종료 hotkey 만 바꿀 수 있다 (CT-06). 그 외 단축키를 바꾸는 기능은 1차 출처에서 못 찾았다.

### 11.3 Clipboard 공유

- 지원 범위:
  - text 와 image 를 양방향으로 자동 동기화한다. 양쪽 모두 켜야 동작한다 (F-01, F-02).
  - 파일 clipboard 는 없다 (F-05).
- 보안 쪽 설계:
  - supporter 쪽 창이 focus 일 때만 동기화한다 (15.73.3).
  - host 사용자가 세션 중에 끌 수 있다 (15.70.3).
- 보조 기능:
  - One-time send (F-03).
  - Paste as keystrokes (F-04): 붙여넣기가 막힌 칸에 쓴다.
- MVP 제안:
  - text 동기화 + focus 조건 + 양쪽 on/off 를 A 로 둔다.
  - image 와 paste-as-keystrokes 는 B 로 둔다.
  - 파일 clipboard 는 TeamViewer 도 하지 않으므로 C 로 둔다.

### 11.4 해상도

- controller 쪽 표시: Scaling Best fit / Original / Scaled + 전체 화면 (V-04, A).
- host 쪽 해상도 바꾸기: Screen Resolution (V-05, B).
- 전송 품질: Auto select 가 기본 (V-01, A). Optimize speed / Optimize quality / Custom 은 B.
- 보조 기능: Hide wallpaper (V-06), Refresh screen (V-08).
- 모니터 없는 host: TeamViewer 는 Windows 에서 가상 모니터로 해결한다 (V-13, C, driver 필요).
- "해상도 이슈 전부"라는 요청에는 V-04 + V-05 + V-01 을 MVP 범위로 보는 것이 맞다. DPI 처리 같은 host 쪽 세부 사항은 host-platform 조사 문서에서 다룬다.

### 11.5 멀티 모니터

- TeamViewer 는 멀티 모니터를 Free tier 부터 제공한다 ([PD-remote]).
- 단계 제안:
  1. 모니터 전환 (V-10, A)
  2. 전체 모니터를 한 창에 (V-11, B)
  3. 모니터마다 창 / tab (V-12, C)
  4. 가상 모니터 추가 (V-13, C)
- 사용자는 멀티 모니터를 후속 기능으로 봤다. 하지만 전환 기능이 없으면 보조 모니터에 있는 창에 아예 갈 수 없으므로, 1단계는 MVP 에 넣는 것을 권한다.

### 11.6 보안 기준선

TeamViewer 가 공개한 내용 중 작은 P2P 도구가 그대로 가져올 수 있는 최소 세트 (모두 A):

- 기기마다 key pair 를 두고 ID 를 검증한다 (S-03).
- 양쪽이 서로 인증하는 E2E 암호화를 쓴다. TLS 1.3 수준, PFS, AES-256-GCM 이고, relay 는 내용을 못 읽는다 (S-01, S-02).
- password 는 PAKE(SRP 류)로 확인해서 서버에 password 와 같은 값이 가지 않게 한다 (S-04).
- 실패하면 대기 시간을 지수로 늘린다 (S-05).
- host 동의 prompt 를 둔다 (S-06).
- stealth mode 를 두지 않고, 세션 중임을 항상 표시한다 (S-07).
- incoming 연결을 로컬 log 에 남긴다 (S-08).
- random password 는 세션마다 새로 만든다 (C-06).
- protocol 버전을 협상한다 (C-33).

바로 다음 단계 (B):
- fingerprint 표시 (S-09)
- block / allowlist (S-10)
- inactive timeout, 최대 세션 시간 (S-11, S-12)
- 설정 보호 (S-13)
- code signing (S-14)

따라 하지 말아야 할 것:
- LAN 모드에서 key exchange 를 생략하는 방식 ([lan]).
- 동의 확인을 경로마다 따로 구현하는 방식. 2026년 우회 취약점 두 건(TV-2026-1003, TV-2026-1007)의 원인이다 ([bulletins]).

## 출처

본문의 `[label]` 표기는 이 아래의 markdown reference link 정의로 연결된다. 미리보기 화면에서는 정의 줄이 보이지 않으므로 본문의 label 을 누르거나, 이 파일의 원본 text 에서 URL 을 확인한다. 모든 출처는 TeamViewer 소유 도메인(teamviewer.com, community.teamviewer.com, TeamViewer 공식 PDF 저장소)이고, `[helpwire]` 하나만 2차 출처다.

<!-- 라이선스·제품 -->

[PD-remote]: https://www.teamviewer.com/en-us/legal/product-descriptions/teamviewer-remote/
[PD-tensor]: https://www.teamviewer.com/en-us/legal/product-descriptions/teamviewer-tensor/
[PD-one]: https://www.teamviewer.com/en-us/legal/product-descriptions/teamviewer-one/
[PD-rm]: https://www.teamviewer.com/en-us/legal/product-descriptions/teamviewer-remote-management/
[PD-dex]: https://www.teamviewer.com/en-us/legal/product-descriptions/teamviewer-dex/
[PD-ar]: https://www.teamviewer.com/en-us/legal/product-descriptions/teamviewer-assist-ar/
[PD-iot]: https://www.teamviewer.com/en-us/legal/product-descriptions/teamviewer-iot/
[PD-pilot]: https://www.teamviewer.com/en-us/legal/product-descriptions/teamviewer-pilot/
[pricing]: https://www.teamviewer.com/en-us/pricing/overview/
[press-one]: https://www.teamviewer.com/en-us/global/company/press/2025/teamviewer-launches-first-digital-workplace-platform/
[free-kb]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/licenses/licenses-and-features/free-license-feature-overview/
[business-kb]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/licenses/licenses-and-features/business-license-feature-overview/
[personal-use]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-classic/licensing/personal-use/for-personal-use/
[commercial-suspected]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-classic/licensing/personal-use/commercial-use-suspected/

<!-- 연결 -->

[cli]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/for-developers/command-line-parameters/
[ports]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/troubleshooting/ports-used-by-teamviewer/
[lan]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/remote-control/use-teamviewer-remote-in-lan/
[id-pw]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/remote-control/connect-via-id-and-password/
[random-pw]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-classic/remote-control/connection-methods/remote-control-via-random-password/
[otp]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-classic/remote-control/connection-methods/remote-control-via-one-time-password/
[attended]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/remote-control/provide-attended-remote-support/
[qs-classic]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-classic/modules/quicksupport/
[qs]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/modules/quicksupport-and-custom-quicksupport/
[host]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/modules/host-and-custom-host/
[unattended]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/remote-control/provide-unattended-remote-support/
[personal-pw]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-classic/remote-control/connection-methods/remote-control-via-personal-password/
[easy-access]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-classic/remote-control/connection-methods/remote-control-via-easy-access/
[easy-access-deploy]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/deployment/mass-deployment-user-guide/grant-easy-access-to-your-devices-and-device-groups-10-10/
[bookmarked]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/devices/bookmarked-vs-managed-devices/
[dock]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/remote-control/connect-via-device-dock/
[portable]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-classic/modules/portable/
[ft-session]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/remote-control/open-a-file-transfer-session/
[rterm]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/remote-control/access-the-command-prompt-via-remote-terminal-connection/
[vpn]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/remote-control/teamviewer-vpn/
[uac]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/remote-control/control-uac-during-a-teamviewer-session/
[wol]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-remote/remote-control/wake-up-a-device-remotely/
[get-started]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/get-started/get-started-with-teamviewer-remote/
[web-client]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-classic/modules/web-client/
[web-ft]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-classic/remote-control/in-session-features/transfer-files-via-the-webclient/
[browser-share]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/remote-control/start-a-screen-sharing-session-via-browser/
[mobile-mobile]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-classic/mobile/connections-from-mobile-to-mobile-devices/
[meeting]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/meeting/create-and-join-a-meeting/
[contact]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/remote-control/connect-to-a-contact/
[button]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-classic/modules/create-a-teamviewer-button-on-your-website/
[av-fwd]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-tensor/company-device-and-user-management/audio-and-video-forwarding/
[ca]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-tensor/conditional-access/conditional-access/
[agentless]: https://www.teamviewer.com/en/global/company/press/2025/teamviewer-agentless-access-for-industrial-remote-operations/
[agentless-kb]: https://www.teamviewer.com/ams/global/support/knowledge-base/teamviewer-tensor/agentless-access/set-up-an-access-gateway-in-teamviewer/
[latest-version]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-remote/get-started/use-teamviewer-remotes-latest-version/

<!-- 보안 -->

[sec-statement]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/security/security-statement/
[sec-pdf]: https://teamviewer.scene7.com/is/content/teamviewergmbh/teamviewer/central-image-hub/pdf/en/teamviewer-security-technical-overview-en.pdf
[sec-pdf-2017]: https://static.teamviewer.com/resources/2017/07/TeamViewer-Security-Statement-en.pdf
[brute-force]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-classic/security/security-features/brute-force-protection/
[policy-settings]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/devices/policy-settings/
[policies]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/devices/policies/
[restrict-access]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-classic/security/best-practices/restrict-access-for-connections/
[fingerprint]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-classic/security/security-features/teamviewer-fingerprint/
[allowlist]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/security/block-and-allowlist/
[timeout]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-classic/remote-control/out-of-session-features/time-out-inactive-sessions/
[48h]: https://www.teamviewer.com/en/insights/teamviewer-connection-blocked-after-timeout/
[protect-options]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-classic/security/security-features/protect-teamviewer-options/
[log-incoming]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/troubleshooting/log-file-reading-incoming-connection/
[2fa-conn]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-classic/security/security-features/two-factor-authentication-for-connections/
[2fa-account]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-classic/security/multi-factor-authentication/security-codes-and-apps-for-two-factor-authentication/
[2fa-enforce]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-classic/security/multi-factor-authentication/enforce-two-factor-authentication-on-company-members/
[trusted-devices]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-classic/security/trusted-devices/trusted-devices/
[trust-push]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-remote/security/trust-devices-via-push-notification/
[zk-recovery]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-classic/security/general-information/zero-knowledge-account-recovery/
[conn-reports]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-remote/security/connection-reports/
[event-log]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-tensor/security/auditability-event-log/
[splunk]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-classic/integrations/3rd-party-integrations/splunk-integration-connection-reporting/
[security-explained]: https://www.teamviewer.com/en/special/security-explained/
[security-center]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/security/increase-your-security-posture-with-the-security-center/
[ca-approval]: https://www.teamviewer.com/ams/global/support/knowledge-base/teamviewer-tensor/conditional-access/enforce-approval-for-devices-receiving-a-connection/
[byoc]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-tensor/security/bring-your-own-certificate/
[sso]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-tensor-classic/sso/single-sign-on-sso/
[scim]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-tensor/integrations/microsoft-entra-id-integration-scim-configuration/
[sec-key]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-tensor/security/security-key-redirection/
[biometric]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-classic/security/security-features/biometric-protection/
[trust-center]: https://www.teamviewer.com/en/resources/trust-center/industry-leading-security/
[bulletins]: https://www.teamviewer.com/en/resources/trust-center/security-bulletins/
[scam]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-remote/security/teamviewer-and-scamming/

<!-- 세션·제어·키보드·화면 -->

[toolbar]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/remote-control/remote-session-toolbar/
[toolbar-classic-mac]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-classic/remote-control/in-session-features/remote-session-toolbar-on-macos/
[v14-manual]: https://teamviewer.scene7.com/is/content/teamviewergmbh/knowledge-hub/product-manuals/TeamViewer14-Manual-Remote-Control-en.pdf
[clipboard]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-remote/remote-control/control-clipboard-sync-during-remote-support-sessions/
[paste-keys]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/remote-control/paste-the-clipboard-as-keystrokes-during-a-remote-session/
[transfer-files]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-classic/remote-control/in-session-features/transfer-files/
[file-box]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/remote-control/share-files-with-the-file-box/
[printing]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-remote/remote-control/use-teamviewer-printing/
[record]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-classic/remote-control/in-session-features/record-a-remote-session/
[record-policy]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/devices/session-recording-policies/
[record-sftp]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-tensor/security/record-teamviewer-sessions-and-upload-them-to-your-cloud-storage/
[whiteboard]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-classic/remote-control/in-session-features/use-the-whiteboard-in-a-remote-session/
[chat-history]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/remote-control/save-the-chat-history-of-a-teamviewer-session/
[black-screen]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-remote/security/teamviewer-black-screen/
[scripts]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-classic/remote-control/in-session-features/execute-scripts-during-a-remote-session-with-teamviewer-classic/
[session-insights]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/remote-control/generate-session-summaries-with-session-insights/
[tia]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-remote/remote-control/troubleshoot-it-issues-with-tia/
[meeting-control]: https://www.teamviewer.com/en-us/global/support/knowledge-base/other-products/meeting/features-and-settings/remote-control-in-meetings/
[key-commands]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-classic/remote-control/in-session-features/use-key-commands-in-sessions/
[direct-kbd]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/remote-control/forward-local-key-input-with-the-direct-keyboard-mode/
[android-touch]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-classic/mobile/android/touch-interactions-vs-mouse-interactions-on-android/
[multi-monitor]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-classic/remote-control/in-session-features/use-multi-monitor-support/
[virtual-monitor]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-remote/remote-control/add-a-virtual-monitor-to-your-remote-device/
[4k]: https://www.teamviewer.com/en/features/4k-remote-desktop-access/
[high-cpu]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/troubleshooting/resolve-high-cpu-usage/
[helpwire]: https://www.helpwire.app/blog/teamviewer-display-resolution-problems/

<!-- 관리·엔터프라이즈 -->

[device-mgmt]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/devices/the-new-device-management-system-explained/
[custom-fields]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/devices/use-device-custom-fields/
[admin-settings]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-remote/get-started/management-console-integrated-into-teamviewer-s-admin-settings/
[roles]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-remote/company-and-users/roles/
[multitenancy]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-tensor/company-device-and-user-management/multitenancy/
[msi]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/deployment/mass-deployment-user-guide/deploy-teamviewer-host-or-full-client-9-10/
[deploy-guide]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-remote/get-started/the-teamviewer-deployment-and-configuration-guide/
[api]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/for-developers/use-the-teamviewer-api/
[integrations]: https://www.teamviewer.com/en-us/integrations/
[intune-press]: https://www.teamviewer.com/en/global/company/press/2026/teamviewer-enhances-microsoft-intune-integration/
[laps]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-tensor/integrations/getting-started-with-the-windows-laps-integration-in-teamviewer/
[service-queue]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-remote/service-management/service-queue-with-teamviewer-remote/
[service-desk]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/service-management/service-desk/
[dex-press]: https://www.teamviewer.com/en/global/company/press/2024/teamviewer-to-acquire-1e/
[frontline]: https://www.teamviewer.com/en-us/products/frontline/
[embedded]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-tensor-classic/teamviewer-embedded/installation/system-requirements/
[intelligence-press]: https://www.teamviewer.com/en/global/company/press/2025/teamviewer-expands-ai-portfolio-with-teamviewer-intelligence-for-it-support-workflows/
[tia-press]: https://www.teamviewer.com/en/global/company/press/2026/teamviewer-expands-ai-powered-it-troubleshooting-from-guidance-to-governed-action/

<!-- 플랫폼 -->

[supported-os]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-remote/download-and-installation/supported-operating-systems-for-teamviewer-remote/
[mac]: https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/remote-control/remote-control-a-mac/
[mac-sequoia]: https://community.teamviewer.com/English/discussion/137020/important-updates-for-teamviewer-on-macos-15-sequoia
[mac-27]: https://community.teamviewer.com/English/discussion/151698/teamviewer-and-macos-27-golden-gate
[mac-27-mdm]: https://community.teamviewer.com/English/discussion/151697/changes-in-mobile-device-management-mdm-on-macos-27-affecting-teamviewer
[mac-catalina]: https://community.teamviewer.com/English/discussion/65493/teamviewer-is-preparing-for-macos-catalina/p1
[wayland]: https://community.teamviewer.com/English/discussion/122410/teamviewer-support-on-wayland-experimental-state
[black-screen-trouble]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-remote/troubleshooting/black-screen-after-starting-a-connection/
[win-server]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-remote/remote-control/use-teamviewer-remote-on-windows-servers/
[arm]: https://community.teamviewer.com/English/discussion/151403/introducing-the-teamviewer-native-arm-client-for-windows
[old-windows]: https://community.teamviewer.com/English/discussion/140714/stopping-active-servicing-for-older-windows-versions
[android-mfr]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-classic/mobile/android/supported-manufacturers-for-remotely-controlling-android-devices/
[android-universal]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-classic/mobile/android/universal-add-on-for-android/
[ios-share]: https://www.teamviewer.com/en-us/global/support/knowledge-base/teamviewer-classic/mobile/ios/share-the-screen-on-your-ipad-iphone/

<!-- Community 공지·Change Log -->

[d134409]: https://community.teamviewer.com/English/discussion/134409
[d139418]: https://community.teamviewer.com/English/discussion/139418
[d139571]: https://community.teamviewer.com/English/discussion/139571
[d140487]: https://community.teamviewer.com/English/discussion/140487
[d140812]: https://community.teamviewer.com/English/discussion/140812/windows-v15-65-4
[d141015]: https://community.teamviewer.com/English/discussion/141015/end-of-support-for-teamviewer-versions-11-and-12
[d141320]: https://community.teamviewer.com/English/discussion/141320/windows-v15-67-3
[d141877]: https://community.teamviewer.com/English/discussion/141877/windows-v15-69-4
[d142196]: https://community.teamviewer.com/English/discussion/142196/windows-v15-70-3
[d142442]: https://community.teamviewer.com/English/discussion/142442/windows-v15-71-4
[d142871]: https://community.teamviewer.com/English/discussion/142871
[d143237]: https://community.teamviewer.com/English/discussion/143237/windows-v15-75-4
[d143409]: https://community.teamviewer.com/English/discussion/143409
[d143411]: https://community.teamviewer.com/English/discussion/143411/windows-v15-76-3
[d143746]: https://community.teamviewer.com/English/discussion/143746/end-of-support-for-teamviewer-versions-13-and-14
[d151090]: https://community.teamviewer.com/English/discussion/151090
[d151306]: https://community.teamviewer.com/English/discussion/151306
[d151503]: https://community.teamviewer.com/English/discussion/151503
[d151600]: https://community.teamviewer.com/English/discussion/151600/windows-v15-81-5
[d151662]: https://community.teamviewer.com/English/discussion/151662/customize-your-hotkey-to-end-incoming-sessions-in-teamviewer
[d151689]: https://community.teamviewer.com/English/discussion/151689/sunsetting-the-teamviewer-management-console
