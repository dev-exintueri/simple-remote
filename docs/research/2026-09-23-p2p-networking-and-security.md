# P2P 연결, NAT traversal, 보안 조사

- 조회일(retrieval date): **2026-09-23**. 버전과 release 날짜는 이날 GitHub releases API 와 crates.io API 로 확인했다.
- 목적: 원격 제어 프로그램 설계를 위한 배경 조사. 최종 결정은 하지 않고 선택지와 trade-off 만 정리한다.
- 표기 규칙
  - 출처 URL 은 주장 바로 뒤에 붙인다. RFC 는 절 번호를 함께 적는다.
  - **미확인(2차 출처)**: RFC, 공식 문서, 프로젝트 repo 같은 1차 출처에서 찾지 못하고 블로그, 포럼, 검색 요약에서만 본 내용.
  - **추정**: 여러 출처를 조합해 이 문서에서 계산하거나 판단한 내용. 출처 자체의 주장이 아니다.

## 0. 현재 제약 조건 (조사 중 두 번 갱신됨)

처음 요청은 "P2P 우선, 연결 초기화를 돕는 서버는 불가피하면 허용"이었다. 조사 도중 아래처럼 좁혀졌고, 이 문서의 결론은 모두 갱신된 제약 기준이다.

1. **세션 트래픽(화면, 입력, 파일, 클립보드)을 나르는 relay 는 어떤 형태로도 쓰지 않는다.** TURN, Tailscale DERP 방식 relay, iroh relay 의 data 전달, RustDesk hbbr 모두 제외하며, 마지막 fallback 으로도 쓰지 않는다. 이유는 트래픽 비용이다.
2. 서버는 첫 연결을 돕는 용도(signaling / rendezvous)만 허용한다. 서버가 아예 없으면 가장 좋다.
3. host 쪽 사용자(컴퓨터를 잘 모르는 가족)는 port forwarding 같은 설정을 하지 않는다. host 가 해외에 있거나 집 공유기 뒤에 있어도 연결이 쉬워야 한다.
4. viewer 는 항상 기술을 아는 사용자 본인이고, 자기 집 네트워크(공유기 port forwarding, UPnP, IPv6 방화벽)는 직접 설정할 수 있다.
5. hosting 은 무료 tier cloud 와 그 앞의 Cloudflare 정도.

## 1. 핵심 요약

**서버 역할: 꼭 필요한 것과 아닌 것**
- **꼭 필요한 서버는 상황에 따라 0개 또는 1개다.**
  - host 프로그램이 viewer 의 DDNS hostname 을 미리 알고 host 가 viewer 로 밖으로 접속(reverse 연결)만 한다면 서버는 필요 없다. DNS(Cloudflare 무료 DNS) 가 주소록 역할을 한다 (6.3절).
  - 둘 다 NAT 뒤에서 hole punching 을 하려면 주소 교환과 시작 시점을 맞출 **signaling/rendezvous 서버 1개**가 사실상 필수다 (2.1절). Cloudflare Workers + Durable Objects 무료 plan 으로 충분하다 (7.1절).
- STUN(공인 주소 알려 주기)은 hole punching 에 필요하지만 `stun.cloudflare.com` 이 "free and unlimited" 라 직접 운영할 필요가 없다 (7.2절).
- relay 는 제약 1 로 제외. 대가로 "느려도 연결은 됨" 상태가 없어지고 실패가 그대로 드러난다 (10장).

**relay 없이 얼마나 실패하나 (추정)**
- hole punching 만으로는 일반 사용자 연결의 약 10~30% 가 실패한다 (4장). TeamViewer 는 자사 연결의 70% 만 직접 연결된다고 공개했다.
- viewer 가 집에서 port forwarding + DDNS 를 해 두고 host 가 밖으로 접속하는 reverse 연결을 더하면, host 가 가정 망에 있는 한 실패 요인이 거의 없어진다. 남는 실패는 viewer 가 집 밖이거나 viewer ISP 가 CGNAT 일 때, host 가 TLS 검사 proxy 가 있는 기업망일 때다 (10장).

**세 가지 선택지 (11장, 결정은 하지 않음)**
1. **WebRTC stack + Workers signaling + 공개 STUN**: ICE 가 모든 경로를 경주하는 구조가 이미 있고 영상 전송 표준이 갖춰져 있다. 대신 signaling 이 DTLS fingerprint 를 바꿔치기할 수 있어 PAKE 로 묶어야 하고, 기업망용 TLS/WebSocket 경로는 따로 만들어야 한다.
2. **QUIC 기반 (iroh relay 끔, 또는 quinn/noq)**: reverse 연결과 보안 모델(공개키 = ID)이 깔끔하다. 대신 iroh 는 relay 를 끄면 hole punching 도 같이 사라져서 hole punching, TCP 경로, 영상 전송 제어를 직접 만들어야 한다.
3. **RustDesk fork (hbbs 만)**: 기능을 가장 빨리 확보한다. 대신 AGPL-3.0, hbbs 용 VM 상시 운영, 암호화 없이 진행하는 fallback 제거와 PAKE 도입 같은 보안 수정, reverse 연결 모드 신규 구현이 필요하다.

**통념과 다른 사실**
- TCP hole punching 성공률이 UDP(QUIC)와 통계적으로 같았다 (약 70%, libp2p 440만 회 측정, 3.4절).
- iroh 에서 relay 를 끄면 data 중계만 없어지는 것이 아니라 NAT 뒤 두 peer 사이의 hole punching 도 사라진다. hole punching 신호가 relay 로 먼저 성립한 연결 안에서 오가기 때문이다 (5.3절).
- RustDesk 는 서버 key 검증이 안 되면 **암호화 없이** 연결을 진행하는 경로가 코드에 있다. 비밀번호 인증도 PAKE 가 아니다 (9.1절).
- TeamViewer 의 E2E 는 TeamViewer master cluster 가 발급한 인증서를 믿는 구조라, 신뢰 기준점이 운영사 자신이다 (8.5절).
- Moonlight/Sunshine 의 PIN pairing 도 PAKE 가 아니며 PIN 은 4자리다 (9.2절).
- RFB(VNC) 표준인 RFC 6143 이 reverse 연결(viewer 가 5500 에서 listen)을 이미 적어 두었다 (6.1절).
- Cloudflare 무료 plan 의 proxy 는 HTTP/HTTPS port 와 WebSocket 만 나른다. 직접 연결에는 DNS-only record 가 필요하고 그러면 viewer 집 IP 가 공개된다 (6.3절).
- Oracle Always Free 의 Ampere A1 은 현재 문서상 "2 OCPUs and 12 GB" 상당이다. 흔히 인용되는 4 OCPU / 24 GB 와 다르다 (7.3절).
- 한국의 IPv6 사용률은 17.42% 로 세계 평균(약 46~51%)보다 훨씬 낮아, IPv6 직접 연결은 한국 가정끼리 큰 도움이 안 된다 (3.2절).

**사람이 정해야 할 것 / 에이전트가 할 수 있는 것**
- 사람: (1) 세 선택지 중 방향, (2) "좌표 전용 relay"(data 한도가 거의 0 인 relay process) 를 제약 1 위반으로 볼지, (3) viewer 쪽에 DDNS 용 domain 을 둘지, (4) 회사 PC host 지원을 범위에 넣을지(넣으면 TLS/WebSocket 443 경로가 필수).
- 에이전트: 선택된 stack 으로 경로 경주, PAKE 채널 묶기, DDNS 갱신, 실패 원인 분류 UI 의 설계와 prototype.

---

## 2. NAT 뒤의 두 peer 를 잇는 데 물리적으로 필요한 것

### 2.1 인터넷 너머에서 "서버 0개" 연결이 보통 안 되는 이유

- 연결하려면 상대의 공인 `IP:port` 를 알아야 한다. NAT 뒤의 기기는 자기 공인 주소를 스스로 알 수 없고, 바깥 서버가 "네 요청이 이 주소에서 왔다"고 알려 줘야 안다. STUN 의 Binding 응답이 바로 이 일을 한다 ([RFC 8489 §2](https://www.rfc-editor.org/rfc/rfc8489#section-2)).
- 두 peer 가 서로의 주소 후보를 주고받을 통로(side channel)가 필요하다. ICE 표준은 이 통로를 정의하지 않고 "candidate 정보를 교환할 수단을 가진 protocol" 위에서 쓴다고만 한다 ([RFC 8445 §3](https://www.rfc-editor.org/rfc/rfc8445#section-3)). Tailscale 도 peer 들의 `ip:port` 를 맞춰 주는 coordination server 가 필요하다고 설명한다 ([Tailscale, How NAT traversal works, 2020-08-21](https://tailscale.com/blog/how-nat-traversal-works)).
- 대부분의 NAT 와 stateful firewall 은 "밖으로 나가는 연결은 허용, 들어오는 연결은 차단"이 기본값이다 ([Tailscale blog](https://tailscale.com/blog/how-nat-traversal-works)). 그래서 양쪽이 거의 동시에 서로에게 패킷을 보내 구멍을 여는 hole punching 이 필요하고, 그 시작 시점을 맞추려면 실시간 통로가 있어야 한다. libp2p DCUtR spec 은 이 문제를 "the need for rendezvous and synchronization"이라 부르고, 보통 전용 signaling server 로 해결한다고 쓴다 ([libp2p DCUtR spec](https://github.com/libp2p/specs/blob/master/relay/DCUtR.md)).

**서버 없이 되는 경우**

| 상황 | 방법 | 한계 |
|---|---|---|
| 같은 LAN | mDNS 로 서로 찾기 ([RFC 6762](https://www.rfc-editor.org/rfc/rfc6762)). iroh 도 mDNS 비슷한 local discovery 를 옵션으로 제공 ([iroh discovery docs](https://docs.iroh.computer/concepts/discovery)) | 같은 네트워크에서만 |
| 한쪽이 공인 IP 이거나 port forwarding 됨 | 그 주소를 사람이 직접 전달 (direct IP). iroh 의 ticket 은 endpoint id, socket 주소, relay URL 을 담은 문자열이라 이런 수동 전달에 쓸 수 있다 ([iroh discovery docs](https://docs.iroh.computer/concepts/discovery)) | 주소가 바뀌면 다시 전달해야 함. DDNS 로 보완 가능 (6.3절) |
| 양쪽 IPv6 | NAT 는 없지만 stateful firewall 은 남는다. "outbound connections only" 규칙이 그대로 있어서 firewall traversal 과 주소 교환 통로가 여전히 필요 ([Tailscale blog](https://tailscale.com/blog/how-nat-traversal-works)) | 한국 IPv6 사용률 17.42% (3.2절) |
| 사람이 signaling 채널 역할 | 주소 문자열을 메신저로 복사 | hole punching 은 양쪽이 몇 초 안에 동시에 보내야 해서 사람 손으로 맞추기 어렵다 (**추정**) |

### 2.2 STUN, ICE, TURN 은 각각 무엇을 하나

- **STUN** ([RFC 8489](https://www.rfc-editor.org/rfc/rfc8489), RFC 5389 를 대체): "STUN is not a NAT traversal solution by itself. Rather, it is a tool" ([§1](https://www.rfc-editor.org/rfc/rfc8489#section-1)). 서버는 요청이 들어온 source 주소를 `XOR-MAPPED-ADDRESS` 로 돌려준다 ([§2](https://www.rfc-editor.org/rfc/rfc8489#section-2)). STUN 서버가 보는 것은 client 의 공인 `IP:port` 와 요청 시각 정도이고 세션 내용은 지나가지 않는다.
- **ICE** ([RFC 8445](https://www.rfc-editor.org/rfc/rfc8445), RFC 5245 를 대체, Proposed Standard 2018-07): candidate 종류는 host, server-reflexive(STUN 으로 얻은 공인 주소), peer-reflexive(상대와 통신하며 새로 알게 된 주소), relayed(TURN) ([§5.1.1](https://www.rfc-editor.org/rfc/rfc8445#section-5.1.1)). 모든 candidate 쌍을 시험해 되는 쌍을 고른다. RFC 5245 의 aggressive nomination 은 폐지됐다 ([§4](https://www.rfc-editor.org/rfc/rfc8445#section-4)).
- **TURN** ([RFC 8656](https://www.rfc-editor.org/rfc/rfc8656), RFC 5766 과 RFC 6156 을 대체): 두 host 가 모두 "not well behaved" NAT 뒤에 있으면 hole punching 이 실패하므로 relay 로 중계한다 ([§1.2](https://www.rfc-editor.org/rfc/rfc8656#section-1.2)). TURN 은 application data 의 기밀성을 보장하지 않고 "Applications that want end-to-end security should encrypt the data" ([§3.4.6](https://www.rfc-editor.org/rfc/rfc8656)). TCP/TLS 를 지원하는 이유는 "some firewalls are configured to block UDP entirely" ([§3.1.5](https://www.rfc-editor.org/rfc/rfc8656)). 운영 비용은 "a high cost to the provider ... high-bandwidth connection" ([§1.8](https://www.rfc-editor.org/rfc/rfc8656)). **이번 설계에서는 제외.**

### 2.3 NAT 동작 분류와 hole punching 이 실패하는 조건

[RFC 4787](https://www.rfc-editor.org/rfc/rfc4787) 은 NAT 를 두 축으로 나눈다. 옛 "full cone / symmetric" 용어는 실제 동작을 설명하기에 부족해서 쓰지 않는다 ([§3](https://www.rfc-editor.org/rfc/rfc4787#section-3)).

| 축 | 종류 | 뜻 (쉽게) |
|---|---|---|
| mapping ([§4.1](https://www.rfc-editor.org/rfc/rfc4787#section-4.1)) | Endpoint-Independent (EIM) | 목적지가 달라도 같은 공인 port 를 재사용. STUN 으로 알아낸 주소를 상대도 그대로 쓸 수 있음 |
| | Address-Dependent / Address and Port-Dependent (EDM, 흔히 "symmetric NAT") | 목적지마다 다른 공인 port. STUN 서버에게 보인 주소와 상대에게 쓰일 주소가 달라서 단순 hole punching 이 실패 |
| filtering ([§5](https://www.rfc-editor.org/rfc/rfc4787#section-5)) | Endpoint-Independent / Address-Dependent / Address and Port-Dependent | 내가 먼저 보낸 적 있는 상대(주소, 또는 주소+port)의 패킷만 들여보내는 정도 |

- RFC 4787 REQ-1: "A NAT MUST have an 'Endpoint-Independent Mapping' behavior." RFC 4787 은 BCP 이고 장비 제조사가 반드시 따르는 것은 아니어서, 실제로는 지키지 않는 장비가 있다 (측정은 3.5절, 4장).
- **한쪽만 EDM**: Tailscale 은 birthday paradox 를 이용한 port 추측으로 해결한다. hard NAT 쪽이 256개, 쉬운 쪽이 2048개 probe 를 보내면 성공 확률 99.9%, 100 packets/sec 기준 20초 안 ([Tailscale blog](https://tailscale.com/blog/how-nat-traversal-works)).
- **양쪽 모두 EDM**: 같은 조건 20초 뒤 성공 확률 0.01%. 99.9% 에 도달하려면 양쪽이 각각 170,000 probe, 100 packets/sec 로 28분 ([Tailscale blog](https://tailscale.com/blog/how-nat-traversal-works)). 사실상 relay 없이 불가.
- **CGNAT**: ISP 가 여러 가입자를 한 공인 IP 뒤에 묶는 NAT. [RFC 6888](https://www.rfc-editor.org/rfc/rfc6888) (BCP 127) 은 CGN 에게 RFC 4787 준수(REQ-1), endpoint-independent filtering 권장(REQ-7), "Paired" IP pooling(REQ-2), 가입자당 port 수 제한 기능(REQ-4), 가입자가 mapping 을 제어할 protocol 구현 MUST 와 그 protocol 로 PCP 권장(REQ-9)을 요구한다. 실제 측정은 3.5절.
- **UDP 자체를 막는 망**: Tailscale 은 UC Berkeley guest Wi-Fi 가 DNS 를 뺀 모든 outbound UDP 를 막는 것을 관찰했다 ([Tailscale blog](https://tailscale.com/blog/how-nat-traversal-works)). 이런 곳에서는 NAT 기법이 소용없고 TCP 경로가 필요하다.

### 2.4 서버 역할 정리: 무엇이 꼭 필요하고, 각 서버가 무엇을 보나

| 역할 | 필요성 (제약 1~5 기준) | 하는 일 | 서버가 보는 것 |
|---|---|---|---|
| rendezvous / signaling | 인터넷 너머 NAT 뒤 peer 사이에서는 **사실상 필수**. 예외는 viewer 가 고정 주소(DDNS)로 직접 도달 가능하고 host 가 그 주소를 이미 아는 경우 (6장) | 접속 ID 를 현재 주소로 바꿔 주기, candidate/SDP 교환, hole punching 시작 시점 맞추기 | 양쪽 공인 IP, 접속 시각, 접속 ID, SDP 안의 DTLS fingerprint 나 공개키. 키 인증을 따로 하지 않으면 **MITM 가능** (8.1절) |
| STUN (주소 반사) | hole punching 을 하려면 필요. signaling 서버가 겸할 수 있다. 예: iroh relay 는 QAD(QUIC Address Discovery)를 같이 제공 ([iroh-relay README](https://github.com/n0-computer/iroh/tree/main/iroh-relay)), RustDesk hbbs 는 UDP 21116 에서 NAT traversal 을 돕는다 ([RustDesk self-host docs](https://rustdesk.com/docs/en/self-host/)) | 공인 `IP:port` 알려 주기 | client 의 공인 `IP:port` |
| relay (TURN, DERP, iroh relay, hbbr) | **이번 설계에서 제외** | 세션 data 중계 | 암호화된 트래픽, 양쪽 IP, 트래픽 양과 시각 |
| 주소 조회 (iroh DNS/pkarr 등) | 선택 | ID 로 relay URL 과 direct 주소 조회 | ID, 공개한 주소 ([iroh discovery docs](https://docs.iroh.computer/concepts/discovery)) |

### 2.5 무료 공개 인프라와 의존 위험

- **iroh public relay** (n0.computer 운영): 무료, rate limit 있음, "no uptime or performance guarantees", 전 세계 iroh 개발자가 공유, "suitable for development and testing"이고 production 에는 유료 Shared/Dedicated relay 를 쓰라고 한다 ([iroh relays docs](https://docs.iroh.computer/concepts/relays)). 1.0 발표 기준 최근 30일에 2억 개 넘는 endpoint 가 만들어졌다 ([iroh 1.0 blog, 2026-06-15](https://www.iroh.computer/blog/v1)). 어차피 relay 는 제외라 이번 설계와는 관계없다.
- **iroh 기본 주소 조회 서버** `dns.iroh.link` (n0 운영) ([iroh discovery docs](https://docs.iroh.computer/concepts/discovery)).
- 공개 STUN 서버 약관과 Cloudflare STUN 은 7장 참고.

---

## 3. relay 없이 직접 연결 성공률을 올리는 기법

### 3.1 공유기 자동 port mapping: UPnP IGD, NAT-PMP, PCP

- 세 protocol 모두 "WAN port 하나를 내 LAN `ip:port` 로 열어 달라"는 요청과 응답으로 요약된다. 성공하면 공인 주소가 공인 서버처럼 동작해서 상대가 그냥 접속하면 된다 ([Tailscale blog](https://tailscale.com/blog/how-nat-traversal-works)).
  - UPnP IGD: 1990년대 말 기술(XML, SOAP, multicast HTTP over UDP)이라 제대로, 안전하게 구현하기 어렵지만 많은 공유기가 지원한다 ([Tailscale blog](https://tailscale.com/blog/how-nat-traversal-works)).
  - NAT-PMP ([RFC 6886](https://www.rfc-editor.org/rfc/rfc6886), Informational, Apple, 2013-04): Standards Track 인 PCP 로 대체됐다고 RFC 스스로 밝힌다.
  - PCP ([RFC 6887](https://www.rfc-editor.org/rfc/rfc6887), Standards Track): NAT, firewall, IPv6 firewall 에 명시적 mapping 을 만든다. CGN 안에서 쓰도록 설계됐다 ([§1](https://www.rfc-editor.org/rfc/rfc6887#section-1)). RFC 6888 REQ-9 는 CGN 이 PCP 같은 protocol 을 구현하라고 요구한다.
- **얼마나 흔한가 (측정)**: 12만 가정 데이터(HomeNet Profiler, Netalyzr, 2011년)에서 UPnP gateway 가 응답한 가정은 데이터셋별 22%, 47%, 54%, 전체 약 35% 였다. 저자들은 "gateway 가 UPnP 를 구현하지 않았다"는 뜻은 아니며 꺼져 있을 수 있다고 쓴다 ([DiCioccio et al., Probe and Pray: Using UPnP for Home Network Measurements, PAM 2012](https://www.icir.org/christian/publications/2012-pam-upnp.pdf)). 최신 전수 측정은 찾지 못했다.
- **CGNAT 에서는 효과 없음에 가깝다**: 집 공유기에 mapping 을 만들어도 ISP 의 CGN 이 한 겹 더 있다. Tailscale 은 CGNAT ISP 들이 가정용 장비에서 이 protocol 을 꺼 두는 경향이 있다고 쓴다 ([Tailscale blog](https://tailscale.com/blog/how-nat-traversal-works)).
- **라이브러리 지원**: iroh 는 UPnP, PCP, NAT-PMP portmapper 를 기본 feature 로 켜 둔다. 문서 주석에 "UPnP uses SSDP multicast"라서 일부 네트워크(특히 macOS)에서 firewall 경고 창이 뜰 수 있어 끌 수 있다고 적혀 있다 (`iroh/src/portmapper.rs`, [iroh repo](https://github.com/n0-computer/iroh)).
- **효과 (추정)**: host 쪽 공유기가 UPnP/PCP 를 켜 두었고 CGNAT 가 아니면, host 쪽 NAT 종류와 상관없이 viewer 가 host 로 직접 들어갈 수 있다. 이 조건을 채우는 가정이 약 1/3~1/2 이라는 2011년 수치 외에는 근거가 약하다.

### 3.2 IPv6 직접 연결

- **보급률**: Google 이 측정한 IPv6 사용자 비율은 2026-09 중순 기준 약 46~51% (날짜별 변동) ([Google IPv6 statistics 데이터 파일](https://www.google.com/intl/en_ALL/ipv6/statistics/data/adoption.js), 화면: [Google IPv6 Statistics](https://www.google.com/intl/en/ipv6/statistics.html)). 나라별(2026-09-20 기준): 한국 17.42%, 미국 55.54%, 일본 56.89%, 독일 75.40%, 프랑스 86.25%, 인도 68.47% ([per-country 데이터](https://www.google.com/intl/en_ALL/ipv6/statistics/data/worldmap.js)). **양쪽 모두 IPv6 가 있어야** 쓸 수 있으므로, 한국 가정끼리는 효과가 작다.
- **가정용 공유기의 IPv6 firewall**: [RFC 6092](https://www.rfc-editor.org/rfc/rfc6092) (Informational, 2011) 는 IPv6 가정용 gateway 의 기본 보안을 권고한다. UDP 에 대해 "Filtering behavior SHOULD be endpoint independent by DEFAULT" (REC-17), TCP 는 3-way handshake 와 "simultaneous-open mode of operation MUST be supported" (REC-31). 즉 권고대로라면 양쪽이 동시에 보내는 hole punching 이 IPv6 에서는 NAT 없이 통과한다. 다만 권고이고 실제 장비 분포는 찾지 못했다.
- IPv4 NAT 연구에서도 "문제는 NAT 보다 filtering"이며 IPv6 로 가도 기본 firewall 은 남을 것이라는 결론이 있다 ([Halkes & Pouwelse 2011](https://opendl.ifip-tc6.org/db/conf/networking/networking2011-2/HalkesP11.pdf)).
- 기업망에는 IPv6 라도 "outbound connections only" 방화벽이 있다 ([Tailscale blog](https://tailscale.com/blog/how-nat-traversal-works)).

### 3.3 Port 예측 (birthday paradox)

- 2.3절 참고. 한쪽만 EDM 이면 약 20초 안에 99.9%, 양쪽 모두 EDM 이면 사실상 불가 ([Tailscale blog](https://tailscale.com/blog/how-nat-traversal-works)).
- 비용: 수백~수천 개의 UDP packet 을 여러 port 로 뿌린다. 일부 IDS/방화벽이 port scan 으로 볼 수 있다 (**추정**).

### 3.4 TCP simultaneous open

- TCP 는 양쪽이 동시에 SYN 을 보내는 연결 방식을 표준으로 가진다 ([RFC 9293 §3.5](https://www.rfc-editor.org/rfc/rfc9293), "A TCP implementation MUST support simultaneous open attempts"). NAT 에 대해서도 [RFC 5382](https://www.rfc-editor.org/rfc/rfc5382) REQ-2 가 "A NAT MUST handle the TCP simultaneous-open mode", REQ-4 가 원치 않는 inbound SYN 에 최소 6초 동안 응답하지 말라고 요구한다. 이 덕분에 TCP hole punching 이 가능하다.
- 측정: 2005년 NAT Check 에서 UDP hole punching 호환 82% (380개 중 310개), TCP 64% (286개 중 184개) ([Ford, Srisuresh, Kegel, USENIX 2005, §6.2](https://bford.info/pub/net/p2pnat/)). 2025~2026년 libp2p 측정에서는 TCP 와 QUIC 성공률이 둘 다 약 70% 로 통계적으로 구분되지 않았다 ([Trautwein et al., arXiv 2604.12484](https://arxiv.org/abs/2604.12484)). "UDP 가 TCP 보다 훨씬 잘 뚫린다"는 통념과 다르다.
- 쓸모: UDP 가 막힌 망에서 두 번째 수단. 단 양쪽 NAT 가 EIM 이어야 한다는 조건은 UDP 와 같다.

### 3.5 CGNAT 과 휴대폰 망

- 측정(IMC 2016, BitTorrent DHT 와 Netalyzr): CGN 을 쓰는 비율은 eyeball AS(가입자를 인터넷에 붙이는 ISP)의 17~18%, cellular AS 의 90% 넘게. 가장 너그러운 결과로 봐도 symmetric(EDM) mapping 인 비율이 가정용 CPE NAT 는 2% 미만, 비 cellular CGN AS 는 11%, cellular CGN AS 는 약 40%. 미국 주요 이동통신사가 symmetric CGN 을 쓴다 ([Richter et al., A Multi-perspective Analysis of Carrier-Grade NAT Deployment, IMC 2016](https://arxiv.org/abs/1605.05606)).
- 뜻: host 나 viewer 가 휴대폰 hotspot 이나 LTE/5G 망에 있으면 hole punching 이 자주 실패한다. 반대편이 EIM 이면 birthday 기법으로 대부분 살릴 수 있지만 양쪽 다 EDM 이면 relay 없이는 안 된다.
- viewer 가 CGNAT 뒤에 있으면 port forwarding 자체가 불가능하다 ("you can't reconfigure the ISP's CGNAT", [Tailscale blog](https://tailscale.com/blog/how-nat-traversal-works)). reverse connection 설계(6장)의 전제 조건이다.

### 3.6 UDP 가 막힌 망

- 2.3절의 UC Berkeley guest Wi-Fi 사례, RFC 8656 §3.1.5.
- relay 없이 남는 수단은 TCP 경로뿐이다: TCP simultaneous open (3.4절), 또는 한쪽이 TCP 로 listen 하고 다른 쪽이 밖으로 접속 (6장 reverse connection).

### 3.7 기법별 효과 요약

| 기법 | 살리는 경우 | 못 살리는 경우 | 효과 크기 근거 |
|---|---|---|---|
| UDP hole punching (STUN + 동시 전송) | 양쪽 EIM | 한쪽 EDM(추가 기법 필요), 양쪽 EDM, UDP 차단 | 70% ± 7.1% (libp2p, 조건부), 82% (2005) (4장) |
| birthday port 예측 | 한쪽만 EDM | 양쪽 EDM | 이론 계산 99.9% / 20초 (Tailscale) |
| TCP simultaneous open | UDP 차단 + 양쪽 EIM | EDM, TCP 도 막힌 망 | libp2p 측정에서 TCP 도 약 70% |
| UPnP/NAT-PMP/PCP | 그쪽 공유기가 지원하고 CGNAT 아님 | CGNAT, 기능 꺼짐 | 가정의 약 35% 가 UPnP 응답 (2011) |
| IPv6 | 양쪽 IPv6 + firewall 이 EIF 또는 동시 전송 허용 | 한쪽이라도 IPv4 only (한국 17%) | 세계 약 46~51% |
| viewer port forwarding + reverse connection (6장) | viewer 가 공인 IP(비 CGNAT) 또는 IPv6 inbound 가능, host 망이 그 port 로 outbound 허용 | viewer 가 CGNAT, host 가 port 를 막는 기업망 | 측정 자료 없음. 6장 참고 |

---

## 4. hole punching 성공률 측정 자료

"relay 없이 직접 연결이 안 되는 비율"을 직접 잰 공개 자료는 드물다. 아래는 각기 다른 집단을 잰 숫자라 그대로 비교하면 안 된다.

| 출처 | 측정 집단 | 숫자 | 주의점 |
|---|---|---|---|
| [Trautwein et al., arXiv 2510.27500 (2025-10-31)](https://arxiv.org/abs/2510.27500), [arXiv 2604.12484 (2026-04-14)](https://arxiv.org/abs/2604.12484) | IPFS/libp2p 자원봉사 client (39개국, 859개 client 네트워크)가 167개국 85,000+ 네트워크의 peer 에게 440만 회 시도 | DCUtR hole punching 단계 성공률 **70% ± 7.1%**. TCP 와 QUIC 차이 없음. 성공의 97.6% 가 첫 시도 | "relay 예약과 공인 주소 발견이 성공했을 때"의 조건부 수치. 그 앞 단계 실패로 제외된 데이터가 약 29% (§6). 참가자가 "more technically proficient ... less restrictive networks"일 수 있다고 저자 스스로 밝힘 (§4.3.1). NAT 종류별 실패 원인은 측정하지 못함 |
| 위 두 숫자의 조합 | | 전체 시도 중 직접 연결까지 간 비율 약 50% (0.70 × 0.71) | **추정**. 논문이 계산한 값이 아님. libp2p 자체 파이프라인 실패를 포함하므로 하한에 가깝다 |
| [TeamViewer Security Statement (Last Modified 2026-05-29)](https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/security/security-statement/) | TeamViewer 전체 연결 (vendor 발표) | "a direct connection via UDP or TCP is established in **70 percent** of all cases", 나머지는 router network 로 TCP 또는 HTTP tunneling | 측정 방법 비공개. 상용 제품이 쓰는 모든 기법을 쓴 결과라는 점에서 "relay 없으면 약 30% 실패"에 가장 가까운 실사용 수치 |
| [Halkes & Pouwelse, UDP NAT and Firewall Puncturing in the Wild, IFIP Networking 2011](https://opendl.ifip-tc6.org/db/conf/networking/networking2011-2/HalkesP11.pdf) | Tribler BitTorrent client 사용자 (시험 1: 907 peer, 시험 2: 1,531 peer), 841 peer 간 15,545 연결 | 79% 가 직접 접속 불가(NAT/firewall 뒤), 64% 는 hole punching 가능한 유형(EIM), 이들 사이 시도의 80% 넘게 성공. EDM NAT 는 11% | 2010년 전후 자료 |
| [Ford, Srisuresh, Kegel, USENIX 2005](https://bford.info/pub/net/p2pnat/) | 자원봉사자 NAT Check (68개 vendor) | UDP 82%, TCP 64% 호환 | NAT 장비 호환성이지 peer 쌍 성공률이 아님 |
| [Tailscale blog (2020)](https://tailscale.com/blog/how-nat-traversal-works) | 저자 추정 | 기본 기법만 구현해도 "a direct connection over 90% of the time" | 측정이 아니라 "I'd estimate" |
| [iroh hole punching docs](https://docs.iroh.computer/concepts/holepunching) | vendor 문서 | "roughly 9 out of 10 network configurations allow a direct connection" | 측정 방법 비공개 |
| WebRTC TURN 사용 비율 | 업계 발언 | "Most large vendors ... report 20% TURN relay traffic. Some reported over 30%" (Tsahi Levent-Levi), callstats.io 2017 연구 인용 "30% of WebRTC P2P traffic has to use TURN" ([discuss-webrtc thread, 2018-06](https://groups.google.com/g/discuss-webrtc/c/5d_EJwM6iJM/m/JfNuyAmlAwAJ)) | **미확인(2차 출처)**. Google 등 운영사의 1차 공개 수치는 찾지 못했다 |
| [Richter et al., IMC 2016](https://arxiv.org/abs/1605.05606) | Netalyzr, BitTorrent DHT | cellular CGN AS 의 약 40% 가 symmetric | 망 유형별 위험도 판단용 |

**정리 (추정)**: 일반 사용자 집단에서 relay 없이 hole punching 만 하면 **대략 10~30% 의 연결이 실패**한다고 보는 것이 출처들과 맞는다. 낮은 쪽(10%)은 Tailscale, iroh 의 vendor 추정, 높은 쪽(30%)은 TeamViewer 실사용 수치와 libp2p 측정이다. 휴대폰 망, 기업망, 공용 Wi-Fi 비중이 높을수록 높은 쪽에 가깝다.

---

## 5. 후보 전송 stack

### 5.1 비교표

| stack | 언어 / license | 최신 release (2026-09-23 확인) | NAT traversal | relay 없이 쓸 때 | 암호화 | 원격 제어 적합성 |
|---|---|---|---|---|---|---|
| libwebrtc (Google) | C++ / BSD-3-Clause ([LICENSE](https://webrtc.googlesource.com/src/+/main/LICENSE)) | release branch 가 Chrome milestone 을 따름 ([native code docs](https://webrtc.github.io/webrtc-org/native-code/development/)) | full ICE, TURN client | STUN 만 설정 가능 (5.3절) | DTLS-SRTP, DTLS/SCTP data channel 필수 ([RFC 8827 §6.5](https://www.rfc-editor.org/rfc/rfc8827)) | 영상 codec, congestion control, jitter buffer 까지 다 들어 있어 가장 완성도 높음. 대신 depot_tools/gn/ninja 빌드, 체크아웃 수 GB 로 무겁다 ([native code docs](https://webrtc.github.io/webrtc-org/native-code/development/)) |
| webrtc-rs | Rust / MIT 또는 Apache-2.0 | `webrtc` v0.21.0 (2026-09-19) | full ICE, TURN | 가능 | DTLS | Sans-I/O core(`rtc` crate) 위에 async layer 를 새로 얹은 구조. 0.x 라 minor bump 에도 breaking change 가능, 0.21 은 1.0 준비 단계 ([README](https://github.com/webrtc-rs/webrtc)) |
| str0m | Rust / MIT 또는 Apache-2.0 | 0.23.1 (crates.io, 2026-08-21) | ICE agent 는 있으나 socket, NIC 조회, TURN 은 app 몫 | app 이 `add_local_candidate` 로 candidate 를 직접 넣는 구조라 **viewer 의 port forwarding 주소를 candidate 로 넣기 쉬움**. README 가 IPv4/IPv6, UDP/TCP 지원을 밝힘 | DTLS | Sans-I/O. SFU 용도로 주로 테스트됐고 "peer-2-peer ... has received less testing" ([README](https://github.com/algesten/str0m)) |
| Pion | Go / MIT | v4.2.20 (2026-09-04) | "Full ICE Agent", ICE restart, trickle, TURN(UDP/TCP/DTLS/TLS), mDNS candidate ([README](https://github.com/pion/webrtc)) | ICE-TCP active/passive 모두 구현(`pion/ice` `tcptype.go`, `WithDisableActiveTCP`), 외부 주소를 candidate 로 공개하는 `SetICEAddressRewriteRules`(구 `SetNAT1To1IPs`), 단일 UDP port mux(`SetICEUDPMux`) ([settingengine.go](https://github.com/pion/webrtc/blob/main/settingengine.go)) | DTLS 1.2 | 순수 Go, 크로스 컴파일 쉬움. Windows 화면 캡처/인코딩은 별도 |
| libdatachannel | C++ (C API) / MPL-2.0 (0.18부터) | v0.24.5 (2026-06-12) | ICE 는 libjuice(기본) 또는 libnice | libjuice 는 "Only UDP is supported as transport protocol" ([libjuice README](https://github.com/paullouisageneau/libjuice)) → TCP 경로가 필요하면 libnice backend | DTLS, SRTP | 가볍고 Windows/macOS 지원, data channel + media transport + WebSocket ([README](https://github.com/paullouisageneau/libdatachannel)) |
| quinn | Rust / MIT 또는 Apache-2.0 | 0.11.12 (crates.io, 2026-09-14) | 없음 | hole punching 을 직접 구현해야 함 | QUIC(TLS 1.3) | crates.io 누적 다운로드 3억 회 이상으로 널리 쓰임 |
| iroh (n0-computer) | Rust (+ Python, Node.js, Swift, Kotlin binding) / MIT 또는 Apache-2.0 | v1.2.0 (GitHub release 2026-09-11, crates.io 2026-09-09). 1.0.0 은 2026-06-15 | hole punching + relay fallback, UPnP/PCP/NAT-PMP portmapper 기본 on | 5.3절: **relay 를 끄면 NAT 뒤 두 peer 사이 hole punching 도 사라짐** | QUIC/TLS, Ed25519 공개키가 곧 EndpointId (`iroh-base/src/key.rs` 가 `ed25519_dalek` 사용) | 1.0 부터 API 와 wire protocol 안정 보장 ("Any change that affects the wire stability ... will always coincide with a major release", [iroh 1.0 blog](https://www.iroh.computer/blog/v1)). QUIC 구현은 quinn 을 hard fork 한 noq (multipath, QUIC NAT traversal draft) ([noq 발표, 2026-03-19](https://www.iroh.computer/blog/noq-announcement)). 1.1.0 에서 악성 relay packet 으로 CPU 100% 를 만드는 버그 등 보안 수정 ([iroh 1.1.0 blog](https://www.iroh.computer/blog/iroh-1-1-0)) |
| rust-libp2p / go-libp2p | Rust, Go / MIT | rust v0.57.0 (2026-09-11), go v0.50.0 (2026-09-21) | DCUtR (relay 연결 위에서 hole punching 동기화), circuit relay v2, AutoNAT | 5.3절 | Noise 또는 TLS | 범용 P2P framework 라 원격 제어에는 기능이 넘친다 (**추정**) |
| Tailscale 계열 (WireGuard + DERP) | Go / BSD-3-Clause (tailscale v1.102.4, 2026-09-10), 자체 control server 는 headscale (BSD-3-Clause, v0.29.3, 2026-07-29) | | hole punching + DERP relay | DERP 는 relay 라 제외. DERP 는 "blindly forwards already-encrypted traffic" ([Tailscale KB](https://tailscale.com/kb/1232/derp-servers)) | WireGuard | VPN 을 통째로 까는 구조라 가족 host 에 설치 부담이 큼 (**추정**). 참고용 |

### 5.2 stack 별 메모

- **WebRTC 계열의 공통 장점**: ICE 가 여러 경로(host, srflx, IPv6, TCP)를 동시에 시험하고 되는 것을 고르는 구조를 이미 갖췄다. 영상 전송용 RTP/RTCP, congestion control, 손실 복구(NACK)가 표준에 있다.
- **WebRTC 계열의 공통 단점**: signaling 서버를 거친 SDP 의 fingerprint 를 따로 인증하지 않으면 signaling 서버가 MITM 할 수 있다 (8.1절).
- **QUIC 계열**: 연결 하나에 stream 여러 개(영상, 입력, 파일, 클립보드)와 datagram 을 담을 수 있다 ([iroh README](https://github.com/n0-computer/iroh)). 영상용 congestion control 과 jitter 처리는 직접 만들어야 한다 (**추정**).

### 5.3 relay 를 끄면 각 stack 에서 무슨 일이 생기나

- **WebRTC (STUN 만)**: `RTCConfiguration.iceServers` 기본값은 빈 배열이고 STUN 서버만 넣어도 된다 ([W3C WebRTC Recommendation 2025-03-13](https://www.w3.org/TR/webrtc/)). relayed candidate 가 없을 뿐 나머지 ICE 동작은 같고, 모든 candidate 쌍이 실패하면 ICE 가 실패로 끝난다. 추가로 ICE-TCP([RFC 6544](https://www.rfc-editor.org/rfc/rfc6544)) passive candidate 를 viewer 의 port forwarding 주소에 두면 host 가 TCP 로 밖으로 접속하는 경로를 ICE 안에 넣을 수 있다 (Pion 은 지원, libjuice 는 미지원).
- **iroh (`RelayMode::Disabled`)**: 문서 주석은 "Disable relay servers completely. This means that neither listening nor dialing relays will be available" (`iroh/src/endpoint.rs`, [repo](https://github.com/n0-computer/iroh)). iroh 연결은 보통 home relay 를 통해 먼저 성립한 뒤 hole punching 으로 direct path 를 찾는다. NAT traversal 주소 후보도 이미 성립한 QUIC 연결 안에서 교환된다 (`iroh/src/lib.rs` "Connection Establishment", `socket/remote_map/remote_state.rs` 의 `n0_nat_traversal` event). 그래서 relay 를 끄면 **"If one of the iroh endpoints can be reached directly"** 인 경우만 연결된다 (`lib.rs`). 즉 viewer 가 port forwarding 된 reverse connection 설계와는 맞지만, 둘 다 NAT 뒤일 때의 hole punching 은 app 이 직접 만들어야 한다.
  - 회색지대 변형 (**추정**): 자체 `iroh-relay` 를 띄우고 `ClientRateLimit`(client 에서 읽는 bytes/sec 상한, [docs.rs iroh-relay 1.2.0](https://docs.rs/iroh-relay/latest/iroh_relay/server/struct.ClientRateLimit.html))을 아주 낮게 걸어 handshake 와 hole punching 신호만 지나가게 하고, app 은 `Connection::paths()` 에서 `is_relay()` 가 아닌 path 가 생기기 전에는 세션 data 를 보내지 않는다. relay process 는 존재하지만 data 비용은 거의 0. 사용자 제약 1 의 "relay 금지"와 충돌하는지 사용자 판단이 필요하다.
  - `Endpoint::bind_addr` 로 port 를 고정하고 `EndpointAddr` 에 viewer 의 공인 주소를 넣어 dial 할 수 있다 (`iroh-base/src/endpoint_addr.rs`).
  - 자체 transport 는 `add_custom_transport` 로 넣을 수 있지만 datagram transport 만 대상이고 "the custom transport API is unstable and will remain so for some time even after iroh 1.0" ([iroh 0.97 blog, 2026-03-16](https://www.iroh.computer/blog/iroh-0-97-0-custom-transports-and-noq)). iroh 로 TCP reverse 경로를 만들려면 이 불안정 API 를 써야 한다.
- **libp2p**: DCUtR 은 "peers start with a relay connection and synchronize directly, without the use of a signaling server" ([DCUtR spec](https://github.com/libp2p/specs/blob/master/relay/DCUtR.md)). 이 relay 는 circuit relay v2 의 "limited relaying" 으로, relay 가 duration 과 data 한도를 알리고 넘으면 stream 을 끊는다 ([circuit v2 spec](https://github.com/libp2p/specs/blob/master/relay/circuit-v2.md)). 기본 한도는 go-libp2p 와 rust-libp2p 모두 **2분, 방향당 128 KiB** ([go resources.go](https://github.com/libp2p/go-libp2p/blob/master/p2p/protocol/circuitv2/relay/resources.go), [rust behaviour.rs](https://github.com/libp2p/rust-libp2p/blob/master/protocols/relay/src/behaviour.rs)). 설계 의도가 "좌표 전용"이라 세션 data 비용은 막히지만, relay 서버 역할 자체는 남는다. iroh 변형과 같은 회색지대.
- **RustDesk**: 문서 흐름은 "attempts to connect A and B directly to each other using hole punching. If hole punching fails, A will communicate with B via the relay server" ([self-host docs](https://rustdesk.com/docs/en/self-host/)). hbbr 를 띄우지 않으면 hole punching 실패 시 연결이 안 된다 (**추정**, 코드로 동작 확인은 안 함).

---

## 6. Reverse connection: viewer 가 listen 하고 host 가 밖으로 접속

발상: 가족 지원에서는 viewer 가 항상 기술을 아는 사용자 본인이다. viewer 쪽을 한 번만 "밖에서 들어올 수 있게" 만들어 두면(자기 공유기 port forwarding, UPnP/NAT-PMP/PCP, 또는 IPv6 + inbound 방화벽 규칙), host 는 밖으로 나가는 연결 하나만 하면 된다. 대부분의 NAT/방화벽이 outbound 는 허용하므로 host 쪽 NAT 종류와 상관없이 연결된다.

### 6.1 선례

- **RFB 표준 자체에 있다**: "In some cases, the initial roles of the client and server are reversed, with the RFB client listening on port 5500, and the RFB server contacting the RFB client. Once the connection is established, the two sides take their normal roles" ([RFC 6143 §2](https://www.rfc-editor.org/rfc/rfc6143#section-2)). 서술 한 문단뿐이고 reverse 전용 보안 절차는 없다.
- **UltraVNC SingleClick (SC)**: "a small (300kB) UltraVNC Server that can be customized and preconfigured ... The connection is initiated by the server to a listening viewer, to allow easy access through customers firewall" ([UltraVNC SC](https://uvnc.com/docs/ultravnc-sc.html)). 접속할 viewer 주소를 실행 파일 설정(`helpdesk.txt`, 예: `-connect 192.168.1.102:5500`)에 넣어 배포한다 ([helpdesk.txt syntax](https://uvnc.com/docs/ultravnc-sc/74-ultravnc-sc-helpdesk-txt-syntax.html)). 암호화는 DSM plugin(현재 SecureVNCPlugin, "2048-bit RSA keys and 256-bit AES keys")을 켜야 한다 ([SecureVNC](https://uvnc.com/downloads/encryption/87-securevnc.html)). 서버 쪽 트레이 메뉴 "Add New Client" 는 "Initiate Connection" 대화상자를 연다 ([winvnc.rc](https://github.com/ultravnc/UltraVNC/blob/main/winvnc/winvnc/winvnc.rc)). 최신 1.8.3.0 ([release](https://uvnc.com/downloads/ultravnc/172-ultravnc-1-8-3-0.html)).
  - 소스를 읽은 결과(subagent 조사, 코드 해석): "Authentication required for server initiated connections" 설정의 기본값이 SC 빌드에서는 false 라서, 기본 SC 는 VNC 비밀번호 없이 밖으로 접속한다 ([SettingsManager.cpp](https://github.com/ultravnc/UltraVNC/blob/main/winvnc/winvnc/SettingsManager.cpp), [vncclient.cpp](https://github.com/ultravnc/UltraVNC/blob/main/winvnc/winvnc/vncclient.cpp)).
- **TightVNC**: "Attach Listening Viewer... Connects from the TightVNC server to a viewer started in the 'listening' mode ... This so called 'reverse connection'", 쓰임새는 "when the server is 'hidden' in local network behind a router" ([TightVNC Getting Started PDF](https://www.tightvnc.com/doc/win/TightVNC_for_Windows-Installation_and_Getting_Started.pdf)). 기본 port 5500, "there is no notification, if connection can not be established" ([command-line options PDF](https://www.tightvnc.com/doc/win/TightVNC_2.7_for_Windows_Server_Command-Line_Options.pdf)). 비밀번호 외 traffic 은 암호화하지 않는다 ([FAQ](https://www.tightvnc.com/faq.php)).
- **TigerVNC**: `vncviewer -listen [port]` 는 "listen on the given port (default 5500) for reverse connections", Xvnc 쪽은 `vncconfig -connect host[:port]` ([vncviewer](https://tigervnc.org/doc/vncviewer.html), [vncconfig](https://tigervnc.org/doc/vncconfig.html)). 최신 v1.16.2 (2026-03-26). 소스상 listen 모드는 처음 들어온 TCP 연결을 주소 필터 없이 받는다 ([vncviewer.cxx](https://github.com/TigerVNC/tigervnc/blob/master/vncviewer/vncviewer.cxx), subagent 코드 해석).
- **Remote Utilities "Callback connection"**: "the Host 'pings' the Viewer and offers remote access. To initiate a remote session, the Viewer only needs to accept this invitation". viewer 쪽에서 허용 IP 를 제한할 수 있고 viewer PC 방화벽에 port 를 열어야 하며, 기본은 수동 수락 ([Callback connection](https://www.remoteutilities.com/support/docs/callback-connection/)). 우리 발상과 가장 가깝다.
- **RustDesk**: host 가 viewer 에게 밖으로 접속하는 모드는 찾지 못했다. 반대 방향인 "Enable direct IP access"(`direct-server`, 기본 N, port `direct-access-port` 기본 21118)는 **host 쪽**이 listen 하는 기능이다 ([advanced settings](https://rustdesk.com/docs/en/self-host/client-configuration/advanced-settings/)).
- **MeshCentral**: agent 가 서버로 "connect back" 하며 이유를 "similar to how browsers connect to web servers ... only one port (HTTPS 443) is needed", "some corporation have firewalls that restrict outgoing connections to only port 80 and 443" 라고 쓴다 ([design](https://docs.meshcentral.com/design/), [user guide](https://docs.meshcentral.com/meshcentral/)). 우리 설계에서 viewer 가 MeshCentral 서버 자리에 서는 셈이다.
- **Microsoft Quick Assist**: 양쪽 모두 443 으로 Microsoft 서비스에 접속하는 relay 방식 ([Microsoft Learn](https://learn.microsoft.com/en-us/windows/client-management/client-tools/quick-assist)). 참고용.

**선례에서 얻는 교훈 (추정)**
- 연결 방향을 뒤집어도 인증 방향은 뒤집히지 않는다. RFB 는 연결 뒤 host 가 viewer 를 확인할 뿐 viewer 가 host 를 확인하는 단계가 없다. 양방향 인증을 따로 넣어야 한다 (8장).
- listening viewer 는 port 에 도달한 누구든 받는다. viewer 의 화면 데이터 parser 버그가 실제로 공격에 쓰였다: TigerVNC 1.10.1 은 Kaspersky 가 찾은 문제로 "could theoretically allow an malicious peer to take control over the software on the other side" 를 고친 보안 release 였다 ([TigerVNC v1.10.1](https://github.com/TigerVNC/tigervnc/releases/tag/v1.10.1)). Kaspersky ICS CERT 는 VNC 구현들에서 CVE 37개를 보고했다 ([Kaspersky ICS CERT 2019](https://ics-cert.kaspersky.com/publications/reports/2019/11/22/vnc-vulnerability-research/), **미확인(2차 출처)**: vendor 가 아닌 제3자 보고서).
- 연결 실패를 host 사용자에게 알려야 한다 (TightVNC 는 알리지 않는다).

### 6.2 임의의 네트워크에서 밖으로 나가는 연결이 얼마나 잘 되나

**UDP 차단 비율 (측정)**
- Google QUIC 논문(SIGCOMM 2017, 2016-11 video client 측정): "QUIC is successfully used for 95.3% of video clients ... 4.4% of clients are unable to use QUIC, meaning that QUIC or UDP is blocked or the path's MTU is too small ... commonly found in corporate networks ... We have not seen an entire ISP blocking QUIC or UDP". 0.3% 는 UDP 속도 제한 망 ([Langley et al., The QUIC Transport Protocol, §7.2](https://static.googleusercontent.com/media/research.google.com/en//pubs/archive/46403.pdf)).
- 같은 논문 §7.5: 첫 QUIC packet 만 통과시키고 나머지를 막은 방화벽이 TCP fallback 판단을 속였다. 부분 차단이 전체 차단보다 위험하다.
- [RFC 9308 §2](https://www.rfc-editor.org/rfc/rfc9308#section-2) (2022-09): "Measurement studies have shown between 3% ... and 5% ... of networks block all UDP traffic", QUIC 위 앱은 실패를 받아들이거나 다른 transport 로 fallback 해야 한다.

**기업망: TCP 443 과 그 밖의 port**
- Microsoft Teams 는 TCP 80/443 과 UDP 3478-3481 을 요구하고, "as a last resort, media can use TCP/IP and also be tunneled within the HTTP protocol, but it isn't recommended due to bad quality implications" ([Teams call flows](https://learn.microsoft.com/en-us/microsoftteams/microsoft-teams-online-call-flows)). Microsoft 365 endpoint 목록 (2026-08-14 갱신) ID 11/12 ([URLs and IP ranges](https://learn.microsoft.com/en-us/microsoft-365/enterprise/urls-and-ip-address-ranges)).
- Tailscale 은 coordination server 와 DERP relay 를 HTTPS 443 으로 쓰고, UDP 41641, 3478 이 막히면 DERP 로 간다 ([Tailscale KB firewall ports](https://tailscale.com/kb/1082/firewall-ports)).
- Zscaler: 기본 firewall 규칙이 "blocks all traffic from your network to the internet", 권장 허용은 HTTP(80), HTTPS(443), DNS(53) ([Recommended Firewall Control Policy](https://help.zscaler.com/zia/recommended-firewall-control-policy)). "Zscaler best practice is to block QUIC" ([Managing the QUIC Protocol](https://help.zscaler.com/zia/managing-quic-protocol)).
- Palo Alto: "It's important to block non-standard port usage, even for web-browsing traffic, because it is an evasion technique" ([Best practice step 4](https://docs.paloaltonetworks.com/content/techdocs/en_US/best-practices/internet-gateway-best-practices/best-practice-internet-gateway-security-policy/define-the-initial-internet-gateway-security-policy/step-4-create-the-temporary-tuning-rules.html)), "Blocking QUIC forces the browser to fall back to TLS and enables the firewall to decrypt the traffic" ([step 3](https://docs.paloaltonetworks.com/content/techdocs/en_US/best-practices/internet-gateway-best-practices/best-practice-internet-gateway-security-policy/define-the-initial-internet-gateway-security-policy/step-3-create-the-application-block-rules.html)). App-ID 는 "irrespective of port, protocol, encryption" 으로 앱을 식별하고 ([App-ID overview](https://docs.paloaltonetworks.com/pan-os/11-1/pan-os-admin/app-id/app-id-overview)), best practice 는 "don't allow unknown tcp, udp, or non-syn-tcp traffic" (step 4). 즉 **443 위의 자체 평문 protocol 은 막힐 가능성이 높다**.
- 호텔, guest Wi-Fi 의 차단율 측정 자료는 찾지 못했다.

**TLS 검사 proxy 의 영향**
- Zscaler 는 동적으로 MITM 인증서를 발급하므로 "certificate-pinned clients might not be able to match those certificates ..., leading to a termination of the connection" 이고, 해결책은 관리자의 검사 예외 설정뿐이다 ([Certificate Pinning and SSL/TLS Inspection](https://help.zscaler.com/zia/certificate-pinning-and-ssl-inspection)). 정책에 따라 신뢰할 수 없는 서버 인증서, SNI 없는 연결, mutual TLS 같은 "Undecryptable Traffic" 을 막을 수 있다 ([SSL Inspection policy](https://help.zscaler.com/zia/configuring-ssl-inspection-policy)).
- Palo Alto: pinned certificate, mutual authentication 등은 "attempting to decrypt the traffic results in blocking the traffic", 예외 목록에 없으면 차단 ([Decryption Exclusions](https://docs.paloaltonetworks.com/pan-os/11-1/pan-os-admin/decryption/decryption-exclusions)).
- Parsec 도 연결 조건에 "SSL traffic must not be decrypted using SSL inspection" 을 둔다 ([Parsec Connectivity Requirements](https://support.parsec.app/hc/en-us/articles/32381460716180-Parsec-Connectivity-Requirements)).

**정리 (추정)**
- 가정, 해외 가정 망: outbound 는 거의 막히지 않는다. host 가 이런 곳에 있으면 reverse 연결은 port 와 상관없이 대부분 된다.
- 기업, 학교 망: TCP 443 만 확실하다. 443 위에서도 (1) 공개 신뢰 인증서를 가진 정상 TLS 여야 하고, (2) TLS 검사 proxy 가 있으면 자체 인증서 pinning 은 끊기므로 **TLS 는 proxy 가 풀도록 두고 그 안에 앱 계층 E2E 암호화를 따로 두어야** 한다. 이 경우 WebSocket over HTTPS 형태가 가장 무난하다.
- 가족 지원에서 host 는 대부분 가정이므로 기업망 문제는 드물 것이다. 단 host 가 회사 노트북이면 VPN 과 회사 proxy 가 끼어 경로 A 만 남을 수 있다.

### 6.3 서버 없이 viewer 를 찾아갈 수 있게 만들기

**DDNS 를 Cloudflare DNS API 로 (무료 plan)**
- Cloudflare 공식 권장 방식: "Create a script to monitor IP address changes and then have that script push changes to the Cloudflare API" ([Managing dynamic IP addresses](https://developers.cloudflare.com/dns/manage-dns-records/how-to/managing-dynamic-ip-addresses/)).
- API: `PATCH /zones/{zone_id}/dns_records/{dns_record_id}`, 권한 `DNS Write`, `AAAA` record 도 지원, TTL 은 60~86400 초 (Enterprise 만 30초까지) ([DNS record edit API](https://developers.cloudflare.com/api/resources/dns/subresources/records/methods/edit/)). DNS-only record 의 TTL 은 non-Enterprise 최소 60초, Auto 는 300초 ([TTL](https://developers.cloudflare.com/dns/manage-dns-records/reference/ttl/)). API 한도는 token 당 5분에 1200회, 넘으면 5분간 차단 ([API limits](https://developers.cloudflare.com/fundamentals/api/reference/limits/)).
- 주의: 권한 범위가 zone 단위라, viewer PC 에 둔 token 이 새면 그 domain 의 DNS 전체를 바꿀 수 있다 ([API permissions](https://developers.cloudflare.com/fundamentals/api/reference/permissions/), record 단위로 좁히는 방법은 문서에서 찾지 못함). DDNS 전용 domain 을 따로 두는 것이 안전하다 (**추정**).
- 이 방식은 사용자 소유 domain 이 필요하다. viewer 는 사용자 본인뿐이므로 host 프로그램에 viewer 의 hostname 을 미리 넣어 배포할 수 있고, 그러면 **signaling 서버 없이** 첫 연결이 가능하다 (UltraVNC SC 가 viewer 주소를 실행 파일에 넣는 것과 같은 방식). 여러 viewer 나 host 목록 관리가 필요해지면 signaling 서버가 다시 필요하다 (**추정**).

**Cloudflare proxy(orange cloud)로는 안 되는 이유**
- proxy 가능한 port 는 HTTP 80, 8080, 8880, 2052, 2082, 2086, 2095 와 HTTPS 443, 2053, 2083, 2087, 2096, 8443 뿐이다. "Spectrum for all TCP and UDP ports is only available on the Enterprise plan" ([Network ports](https://developers.cloudflare.com/fundamentals/reference/network-ports/)). Free plan 은 Spectrum 이 "No" ([Protocols per plan](https://developers.cloudflare.com/spectrum/protocols-per-plan/)).
- WebSocket 은 "supported on all Cloudflare plans" 이지만 Cloudflare 가 코드를 배포할 때 서버를 재시작하며 연결을 끊을 수 있다 ([WebSockets](https://developers.cloudflare.com/network/websockets/)).
- 즉 viewer 집으로 가는 경로를 orange cloud 로 두면 모든 세션 traffic 이 Cloudflare 를 지나가는 relay 가 된다(제약 1 위반, 게다가 WebSocket 만 가능). **직접 연결에는 DNS-only(grey cloud) record 가 필요**하다. DNS-only record 는 "exposes your origin IP address to anyone who queries the record" ([Proxy status](https://developers.cloudflare.com/dns/proxy-status/)), 즉 viewer 집 IP 가 공개된다.
- 인증서: TLS 검사 proxy 를 통과하려면 공개 신뢰 인증서가 필요하다(6.2절). Let's Encrypt 의 DNS-01 challenge 는 TXT record 로 domain 소유를 증명하고, port 80 을 열 필요가 없으며 wildcard 도 된다 ([Let's Encrypt challenge types](https://letsencrypt.org/docs/challenge-types/)). 같은 Cloudflare API token 으로 자동화할 수 있다 (**추정**).

**IPv6 로 viewer 를 여는 경우**
- 가정용 IPv6 gateway 권고([RFC 6092](https://www.rfc-editor.org/rfc/rfc6092))는 들어오는 요청을 막는 stateful filter 를 기본으로 두고, 앱이 inbound 를 요청할 protocol 을 "SHOULD implement" (REC-48), 모든 inbound 를 통과시키는 "transparent mode" 를 쉽게 켤 수 있게 "MUST provide" (REC-49) 한다. 그래서 viewer 는 공유기에서 자기 PC 의 IPv6 주소와 port 에 대한 허용 규칙을 만들거나 PCP 로 요청해야 하고, Windows 방화벽의 inbound 규칙도 따로 필요하다 (**추정**).
- 주의: IPv6 는 개인정보 보호용 임시 주소가 주기적으로 바뀐다 ([RFC 8981](https://www.rfc-editor.org/rfc/rfc8981)). 또 ISP 가 준 prefix 가 재접속 때 바뀔 수 있다 (**추정**). 따라서 고정 주소 대신 DDNS 의 AAAA 기록을 갱신하는 방식이 필요하다.
- 한계: host 쪽도 IPv6 가 있어야 한다. 한국 IPv6 사용률 17.42% (3.2절).

### 6.4 인터넷에 열린 listening viewer 의 보안

- **port 는 금방 발견된다**: ZMap 은 "On a computer with a gigabit connection, ZMap can scan the entire public IPv4 address space on a single port in under 45 minutes" ([zmap.io](https://zmap.io/)). 열어 둔 port 는 누군가 곧 찔러 본다고 전제해야 한다.
- **옛 선례의 약점**: RFB(VNC)의 reverse connection 은 인증이 약한 protocol 위에 있다 ("known to be cryptographically weak", [RFC 6143 §7.2.2](https://www.rfc-editor.org/rfc/rfc6143#section-7.2.2)).
- **대책 (설계 제안, 추정)**
  1. **초대받지 않은 연결에는 응답하지 않기**: WireGuard 는 첫 handshake 메시지에 인증을 넣어 "the server does not even respond at all to an unauthorized client; it is silent and invisible", 인증 전에는 상태도 만들지 않는다 ([WireGuard protocol](https://www.wireguard.com/protocol/)). 같은 발상으로, 첫 packet 에 PAKE 나 pairing 키로 만든 값을 넣고 맞지 않으면 조용히 버린다.
  2. **필요할 때만 열기**: viewer 가 "지원 시작"을 누른 동안만 listener 를 열고, 연결되거나 시간이 지나면 닫는다. UPnP/PCP 로 연 mapping 도 수명을 짧게 둔다 (PCP 는 lifetime 을 가진 mapping 을 만든다, [RFC 6887](https://www.rfc-editor.org/rfc/rfc6887)).
  3. **일회용 코드 + PAKE**: 8.2절. host 화면의 코드를 viewer 가 입력하거나 그 반대. 결과 키로 채널(TLS/DTLS/QUIC)을 묶어 signaling 이나 DNS 를 바꿔치기한 공격을 막는다 (8.1절).
  4. **source IP 별 실패 제한**: 8.4절. QUIC 이면 handshake 전 주소 확인(Retry)으로 위조 source 를 걸러낼 수 있다 ([RFC 9000 §8.1](https://www.rfc-editor.org/rfc/rfc9000#section-8.1)).
  5. **viewer 프로그램 자체의 취약점**: listener 는 인터넷에서 오는 입력을 처리하는 서버가 된다. parser 를 최소화하고 인증 전에는 거의 아무것도 해석하지 않게 한다.

### 6.5 결합 설계: 가능한 경로를 모두 동시에 시도하고 먼저 되는 것을 쓰기

**후보 경로**

| 경로 | 방향 | 필요한 것 | 막히는 경우 |
|---|---|---|---|
| A. host → viewer (reverse), TCP/TLS 443 | host 가 밖으로 | viewer 공유기 port forwarding 또는 UPnP/PCP, viewer 가 공인 IPv4(비 CGNAT) 또는 IPv6 inbound, DDNS | viewer 가 CGNAT 또는 외출 중, host 망의 TLS 검사 proxy (6.2절) |
| B. host → viewer (reverse), UDP (QUIC/DTLS) | host 가 밖으로 | A 와 같음 | host 망이 UDP 차단 |
| C. viewer → host | viewer 가 밖으로 | host 공유기가 UPnP/NAT-PMP/PCP 에 응답 | host 가 CGNAT, UPnP 꺼짐 |
| D. UDP hole punching | 양쪽 동시 | signaling 으로 주소 교환, STUN | 양쪽 EDM, UDP 차단 |
| E. TCP simultaneous open | 양쪽 동시 | D 와 같음 | EDM, OS/NAT 호환성 |
| F. IPv6 직접 | 양쪽 | 양쪽 IPv6, firewall 이 동시 전송 허용 | 한쪽 IPv4 only |

**선례**: Tailscale 은 candidate 목록에 "IPv4 WAN ip:port allocated by a port mapping protocol"과 "Operator-provided endpoints (e.g. for statically configured port forwards)"를 넣고, 모든 후보에 동시에 probe 를 보낸다 ([Tailscale blog](https://tailscale.com/blog/how-nat-traversal-works)). 여러 연결을 시간차를 두고 경주시키는 일반 기법은 Happy Eyeballs ([RFC 8305](https://www.rfc-editor.org/rfc/rfc8305)).

**라이브러리로 "viewer 의 port forwarding 주소"를 TURN 없이 후보로 넣을 수 있나**

| 라이브러리 | 방법 | 비고 |
|---|---|---|
| Pion | `SetICEAddressRewriteRules` (구 `SetNAT1To1IPs`) 로 외부 주소를 host/srflx candidate 로 공개, `SetICEUDPMux` 로 UDP port 고정, `SetICETCPMux` 로 ICE-TCP passive candidate ([settingengine.go](https://github.com/pion/webrtc/blob/main/settingengine.go)). host 쪽은 active TCP candidate 로 밖으로 접속 (`pion/ice` `tcptype.go`) | 경로 B/D/F 와 TCP 경로를 ICE 한 번으로 경주 가능. viewer 의 TCP 443 에 ICE-TCP passive candidate 를 두면 경로 A 와 비슷해지지만 TLS 가 아니므로 TLS 검사 proxy 나 App-ID 는 통과하지 못할 수 있다 (**추정**) |
| str0m | app 이 `add_local_candidate` 로 아무 주소나 candidate 로 추가 ([README](https://github.com/algesten/str0m)) | socket 관리는 app 몫. TCP 지원을 README 가 밝힘 |
| libdatachannel | libjuice 는 UDP 만 ([libjuice README](https://github.com/paullouisageneau/libjuice)) | TCP 경로는 libnice backend 필요 |
| libwebrtc | 확인하지 못함 | 로컬 candidate 를 임의로 넣는 공개 API 여부 미확인 |
| iroh | `EndpointAddr` 에 viewer 의 공인 주소들을 넣고 dial, viewer 는 `bind_addr` 로 port 고정 (`iroh-base/src/endpoint_addr.rs`, `iroh/src/endpoint.rs`) | UDP(QUIC)만. TCP 경로는 불안정 API(custom transport) 필요. 둘 다 NAT 뒤일 때의 hole punching 은 relay 없이는 안 됨 (5.3절) |
| libp2p | listen 주소를 identify 로 알리고 dial. TCP, QUIC transport 가 기본 | DCUtR 은 relay 연결이 전제 (5.3절) |

**이 결합 설계로도 실패하는 경우 (추정)**
1. viewer 가 집 밖(카페, 휴대폰 hotspot, 해외 호텔)에 있어 경로 A/B 가 없고, host 공유기가 UPnP 에 응답하지 않으며(경로 C 실패), 양쪽 중 하나가 EDM 이거나 UDP 가 막혀 hole punching 도 실패하는 경우.
2. viewer 의 ISP 가 CGNAT 이고 IPv6 도 없는 경우: 경로 A/B 가 아예 불가능해 hole punching 에만 의존.
3. host 가 기업망이나 학교망에 있어 TCP 443 외 outbound 가 막히고, 443 에는 TLS 검사 proxy 가 있는 경우 (6.2절).
4. viewer 공인 IP 가 바뀌었는데 DDNS 가 아직 갱신되지 않은 경우 (TTL 동안).

---

## 7. signaling 을 무료로 hosting 하기

### 7.1 Cloudflare Workers + Durable Objects (signaling 용)

- Durable Object(DO)는 이름 하나당 전 세계에 인스턴스 하나만 있는 상태 보관 객체라, "접속 ID 하나 = 방 하나" 로 두 peer 의 WebSocket 을 한곳에 모으는 signaling 에 맞는다 (**추정**, 설계 해석). DO 는 "can act as WebSocket servers that connect thousands of clients per instance" ([Use WebSockets, 2026-06-19 갱신](https://developers.cloudflare.com/durable-objects/best-practices/websockets/)).
- Free plan 제한 ([Durable Objects pricing, 2026-08-25 갱신](https://developers.cloudflare.com/durable-objects/platform/pricing/)):
  - Free plan 에서는 "Only Durable Objects with SQLite storage backend are available".
  - 하루 100,000 requests, 13,000 GB-s duration, SQLite 읽기 500만 행, 쓰기 10만 행, 저장 5 GB.
  - 들어오는 WebSocket message 는 "a 20:1 ratio ... 100 WebSocket incoming messages would be charged as 5 requests".
  - hibernation 가능한 idle 객체는 duration 을 청구하지 않는다.
- Workers Free plan ([Workers limits](https://developers.cloudflare.com/workers/platform/limits/)): 하루 100,000 requests, 넘으면 Error 1027 (또는 fail open 설정 시 Worker 우회), HTTP 요청당 CPU 10 ms.
- 용량 감각 (**추정**): 연결 한 번에 WebSocket 연결 2개와 message 수십 개면 request 로 몇 개 수준이다. 가족 지원 규모(하루 수십 회)는 한도의 1% 도 안 된다.
- Worker 는 요청 header `CF-Connecting-IP` 로 client 공인 IP 를 알 수 있다 ([Cloudflare HTTP headers](https://developers.cloudflare.com/fundamentals/reference/http-headers/)). 하지만 이것은 TCP 연결의 IP 이고 UDP NAT mapping port 는 알 수 없으므로 STUN 을 대신하지 못한다 (**추정**).
- Worker 는 HTTP/HTTPS port 로만 traffic 을 받는다(6.3절 port 목록). **UDP STUN 서버를 Workers 에 올릴 수는 없다.**

### 7.2 STUN 은 어디서

| 선택지 | 조건 | 위험 |
|---|---|---|
| `stun.cloudflare.com:3478/udp` | "Cloudflare's STUN service at stun.cloudflare.com is free and unlimited" ([Realtime TURN FAQ](https://developers.cloudflare.com/realtime/turn/faq/)). alternate port 없음 ([TURN service](https://developers.cloudflare.com/realtime/turn/)) | STUN 전용 약관은 찾지 못함. UDP 3478 을 막는 망에서는 무용 |
| `stun.l.google.com:19302` | webrtc.org 예제 코드에만 등장 ([Peer connections](https://webrtc.org/getting-started/peer-connections)) | SLA 나 이용 약관을 공식 페이지에서 찾지 못함. 예고 없이 막혀도 할 말이 없다 (**추정**) |
| 자체 VM 에서 STUN 운영 | DNS-only record 로 VM IP 를 가리킴 | VM 유지 비용과 회수 정책 (7.3절) |

- 참고로 webrtc.org 는 "For most WebRTC applications to function a server is required for relaying the traffic between peers, since a direct socket is often not possible" 이라고 쓴다 ([webrtc.org TURN server](https://webrtc.org/getting-started/turn-server)). relay 를 뺀다는 결정이 업계 기본값과 반대라는 점을 보여 준다.

### 7.3 무료 VM (signaling + STUN 을 한 대에)

| 제공자 | 무료 조건 (공식 문서) | 이 용도에서의 문제 |
|---|---|---|
| Oracle Cloud Always Free | E2.1.Micro VM 최대 2대(1/8 OCPU, 1 GB, 인터넷 대역 최대 50 Mbps). Ampere A1 은 월 1,500 OCPU 시간과 9,000 GB 시간, "equivalent to 2 OCPUs and 12 GB of memory". outbound 월 10 TB. home region 에서만 ([Always Free Resources](https://docs.oracle.com/en-us/iaas/Content/FreeTier/freetier_topic-Always_Free_Resources.htm)) | 7일 동안 CPU p95, network, (A1 은 memory 도) 사용률이 모두 20% 미만이면 idle 로 보고 회수할 수 있다 (같은 문서). 한가한 signaling 서버는 이 기준에 걸리기 쉽다. 30일 넘게 방치한 계정은 정지될 수 있다 ([Free tier FAQ](https://www.oracle.com/cloud/free/faq/)) |
| Google Cloud Free Tier | us-west1, us-central1, us-east1 중 한 곳에 e2-micro 1대, 표준 PD 30 GB-월, 북미발 outbound 월 1 GB ([Free cloud features, 2026-09-22 갱신](https://cloud.google.com/free/docs/free-cloud-features)) | 외부 IPv4 주소가 시간당 $0.005 이고 무료분은 "limited to one hour per month per account" ([VPC pricing](https://cloud.google.com/vpc/network-pricing)). 한 달 약 $3.65 (**추정**, 0.005 × 730시간). 미국 region 만 가능해 한국에서 RTT 가 크다 |
| AWS | 2025-07-15 부터 신규 계정은 credit 방식. "The free account plan expires after 6 months or when you exhaust your credits" ([AWS News Blog](https://aws.amazon.com/blogs/aws/aws-free-tier-update-new-customers-can-get-started-and-explore-aws-with-up-to-200-in-credits/)). 만료 시 계정 closure ([Free tier FAQ](https://aws.amazon.com/free/free-tier-faqs/)) | 상시 운영에 맞지 않음 |

- 흔히 인용되는 "Oracle A1 4 OCPU / 24 GB 무료"는 현재 공식 문서(2 OCPU / 12 GB 상당)와 다르다.

### 7.4 정리

- **signaling 만** 필요하면 Cloudflare Workers + DO 무료 plan 으로 충분하고 VM 이 필요 없다. STUN 은 `stun.cloudflare.com` 을 쓰면 된다.
- 6.3절처럼 host 가 viewer 의 DDNS hostname 을 미리 알고 reverse 연결만 쓴다면 signaling 서버조차 없어도 첫 연결이 된다. 이때 hole punching 경로(D, E)는 쓸 수 없다(시작 시점을 맞출 통로가 없음).

---

## 8. 보안 기준선

### 8.1 E2E 암호화와 "signaling 서버가 중간에 끼는" 공격

- WebRTC 는 ICE 가 끝난 뒤 DTLS handshake 를 하고, 그 key 로 영상(SRTP)과 data channel(SCTP over DTLS)을 암호화한다 ([RFC 8827 §4.3](https://www.rfc-editor.org/rfc/rfc8827#section-4.3)). "All data channels MUST be secured via DTLS" ([§6.5](https://www.rfc-editor.org/rfc/rfc8827#section-6.5)). 상대 인증서의 fingerprint 는 SDP 의 `a=fingerprint` 로 signaling 을 거쳐 전달된다 ([RFC 8122](https://www.rfc-editor.org/rfc/rfc8122)).
- 그래서 signaling 서버는 **수동적으로 엿보지는 못하지만, 능동적으로 fingerprint 를 바꿔치기할 수 있다.**
  - "Even if HTTPS is used, the signaling server can potentially mount a man-in-the-middle attack unless implementations have some mechanism for independently verifying keys" ([RFC 8827 §9.1](https://www.rfc-editor.org/rfc/rfc8827#section-9.1)).
  - "it can simply mount a man-in-the-middle attack on the connection, telling Alice that she is calling Bob and Bob that he is calling Alice, while in fact the calling service is acting as a calling bridge" ([RFC 8826 §4.3.2](https://www.rfc-editor.org/rfc/rfc8826#section-4.3.2)).
- RFC 가 제시하는 대책과 한계
  - 사람이 fingerprint 나 SAS(short authentication string)를 비교 ([RFC 8827 §6.5](https://www.rfc-editor.org/rfc/rfc8827#section-6.5)). 하지만 "not suitable for general use" ([§9.1](https://www.rfc-editor.org/rfc/rfc8827#section-9.1)). 키 연속성(key continuity) 경고는 사용자가 "trained to simply click through" 한다 ([RFC 8826 §4.3.2.1](https://www.rfc-editor.org/rfc/rfc8826)).
  - 제3자 IdP (Identity Provider) 주장 ([RFC 8827 §7](https://www.rfc-editor.org/rfc/rfc8827#section-7)). 외부 계정 체계가 필요해서 가족 지원 용도와 안 맞다 (**추정**).
- **이 설계에 맞는 대책 (추정, 설계 제안)**: host 화면의 일회용 코드를 PAKE 입력으로 써서 공유 키를 만들고, 그 키로 양쪽이 관찰한 DTLS fingerprint(또는 TLS exporter 값)를 MAC 해 교환한다. 값이 다르면 중간에 누가 끼었다는 뜻이므로 끊는다. TLS 계열에서 이런 "채널 묶기"는 [RFC 5705](https://www.rfc-editor.org/rfc/rfc5705) (keying material exporter) 와 [RFC 9266](https://www.rfc-editor.org/rfc/rfc9266) (Channel Bindings for TLS 1.3) 로 표준화돼 있다. 이렇게 하면 signaling 서버를 믿지 않아도 된다.
  - 같은 발상의 실제 구현: RustDesk 는 WebRTC 경로에서 peer 가 자신의 서명 키로 DTLS fingerprint 에 서명하게 하고, 불일치하면 "possible MITM"으로 끊는다 (`src/client.rs` `secure_connection`, [RustDesk repo](https://github.com/rustdesk/rustdesk)). 단 RustDesk 는 서명 키 자체를 rendezvous 서버 key 로 인증한다 (9.1절).
- **QUIC/iroh 도 같은 구조**: iroh 는 dial 한 공개키(EndpointId)와 실제 상대가 맞는지 TLS 로 확인한다 ([iroh crate docs, `lib.rs` "Encryption"](https://github.com/n0-computer/iroh)). 하지만 그 EndpointId 를 signaling 으로 받는다면 signaling 서버가 바꿔치기할 수 있으므로, EndpointId 도 PAKE 나 pairing 으로 인증해야 한다 (**추정**).
- relay 를 쓰지 않으므로 peer 끼리 서로의 IP 를 알게 된다. RFC 8826 은 callee 가 ICE candidate 를 보내는 순간 caller 가 IP 를 알게 되는 것을 privacy 문제로 든다 ([RFC 8826 §4.2.4](https://www.rfc-editor.org/rfc/rfc8826)). 가족끼리라면 문제가 작지만, 모르는 사람이 접속 ID 만으로 host 의 IP 를 알아내지 못하게 signaling 순서를 짜야 한다 (예: 코드 검증 뒤에 candidate 교환) (**추정**).

### 8.2 attended access (일회용 코드) 인증: 왜 PAKE 인가

- 일회용 코드는 짧아서 추측하기 쉽다. PAKE(Password-Authenticated Key Exchange)는 비밀번호나 비밀번호에서 나온 값을 보내지 않고 양쪽이 같은 비밀을 안다는 것을 증명하며, "Such exchanges are therefore resistant to offline, brute-force dictionary attacks" ([RFC 8125 §1](https://www.rfc-editor.org/rfc/rfc8125#section-1)). 공격자는 "cannot enumerate through her dictionary without interacting with Alice or Bob for each password guess"이고 한 번의 능동 공격으로 "whether her single guess is correct or not"만 알게 된다 (같은 곳).
- 반대로 PAKE 가 아닌 "hash(비밀번호 + challenge)" 방식은 대화 내용을 본 공격자가 가능한 코드를 전부 오프라인으로 대입할 수 있다. 6자리 숫자면 100만 개라 일반 PC 로도 순식간이다 (**추정**, 계산).
- 실제 예: Magic Wormhole 은 기본 16-bit 코드와 SPAKE2 를 써서 "an attacker gets a 1-in-65536 chance of success" 이고, 틀리면 정상 사용자 쪽 연결이 실패해 공격이 드러난다 ([Magic Wormhole docs](https://magic-wormhole.readthedocs.io/en/latest/welcome.html)). 가족 원격 지원의 "코드 불러 주기" 흐름과 거의 같다.
- TeamViewer 도 비밀번호 인증에 SRP-6 를 쓴다 (8.5절).

**PAKE 종류**

| 방식 | 표준 상태 | 종류 | 이 설계에서의 쓰임 |
|---|---|---|---|
| SPAKE2 | [RFC 9382](https://www.rfc-editor.org/rfc/rfc9382) (Informational, 2023-09) | balanced (양쪽이 같은 비밀). "SPAKE2 does not support augmentation" | 일회용 코드 (attended) |
| CPace | [draft-irtf-cfrg-cpace-21](https://datatracker.ietf.org/doc/draft-irtf-cfrg-cpace/) (최신 개정 2026-04-22, IRTF 에서 RFC Editor 로 넘어가 "Awaiting Editor Assignment", Informational 예정) | balanced | 일회용 코드 (attended) |
| OPAQUE | [RFC 9807](https://www.rfc-editor.org/rfc/rfc9807) (2025-07, Informational) | augmented (서버는 비밀번호 대신 검증값만 저장) | unattended 고정 비밀번호를 host 가 보관할 때 |
| SRP | [RFC 2945](https://www.rfc-editor.org/rfc/rfc2945), [RFC 5054](https://www.rfc-editor.org/rfc/rfc5054) | augmented, 오래됨 | TeamViewer 가 사용 |

**유지되는 라이브러리 (2026-09-23 확인)**

| 언어 | 라이브러리 | 상태 |
|---|---|---|
| Rust | `spake2`, `srp`, `aucpace` ([RustCrypto/PAKEs](https://github.com/RustCrypto/PAKEs), MIT 또는 Apache-2.0) | spake2 안정판 0.4.0, 0.5.0-pre.0 (2026-01-25). srp 안정판 0.6.0, 0.7.0-rc.3 (2026-04-03). README: "have not yet received any formal cryptographic and security reviews ... USE AT YOUR OWN RISK" |
| Rust | `opaque-ke` ([facebook/opaque-ke](https://github.com/facebook/opaque-ke), Apache-2.0 또는 MIT) | 안정판 4.0.1, 4.1.0-pre.2 (2026-03-27). RFC 9807 기반. 2021-06 NCC Group 감사 (WhatsApp 후원) |
| Rust | `pake-cpace` ([jedisct1/rust-cpace](https://github.com/jedisct1/rust-cpace), ISC) | 0.1.7, 마지막 갱신 2023-12. 사실상 멈춤 |
| Go | [bytemare/opaque](https://github.com/bytemare/opaque) (MIT) | v0.18.0 (2026-03-24), RFC 9807 |
| Go | [bytemare/cpace](https://github.com/bytemare/cpace) (MIT) | release 없음, 2026-07 push |
| Go | [FiloSottile/cpace](https://github.com/FiloSottile/cpace) (BSD-3-Clause) | "EXPERIMENTAL", 마지막 push 2021 |
| Go | `salsa.debian.org/vasudev/gospake2` | [wormhole-william](https://github.com/psanford/wormhole-william) 의 go.mod 가 2021년 pseudo-version 으로 사용 |
| C/C++ | BoringSSL `SPAKE2_*` (`include/openssl/curve25519.h`, [google/boringssl](https://github.com/google/boringssl)) | 주석: "An attacker can only make one guess of the password per execution". 단 기준이 draft-irtf-cfrg-spake2-02 라 RFC 9382 와 호환되는지 확인 필요 |
| C | [stef/libopaque](https://github.com/stef/libopaque) (LGPL-3.0) | v1.0.1 (2025-02-23) |
| C | Mbed TLS (현재 TF-PSA-Crypto) EC J-PAKE | 헤더 주석: "as defined in Chapter 7.4 of the Thread v1.0 Specification" ([ecjpake.h](https://github.com/Mbed-TLS/TF-PSA-Crypto/blob/development/drivers/builtin/include/mbedtls/private/ecjpake.h)) |

정리: 감사를 받은 것은 opaque-ke 뿐이다. balanced PAKE(일회용 코드용)는 어느 언어든 "감사 안 됨" 상태의 라이브러리를 쓰게 된다.

### 8.3 unattended access: 기기 키, 처음 신뢰(TOFU), pairing, 키 보관

- **기기 키**: 각 기기가 장기 키 쌍을 만들고 공개키를 ID 로 쓴다. iroh 는 EndpointId 가 곧 공개키이고 연결 때 상대가 그 키를 가졌는지 TLS 로 확인한다 ([iroh `lib.rs`](https://github.com/n0-computer/iroh)). AnyDesk 는 첫 실행 때 RSA 인증서와 개인키를 만들고 서버가 ID 와 fingerprint 를 영구 연결한다 ([AnyDesk Fingerprint](https://support.anydesk.com/docs/fingerprint)).
- **pairing (설계 제안, 추정)**: 처음 한 번은 attended 흐름(host 화면 코드 + PAKE)으로 연결하고, 그 보호된 채널 안에서 서로의 장기 공개키를 교환해 저장한다. 이후 unattended 접속은 저장된 공개키로만 인증한다. 코드 없이 처음 본 키를 믿는 순수 TOFU 는 첫 연결 때 signaling 서버의 바꿔치기를 막지 못한다 (8.1절).
- **키 보관**
  - Windows DPAPI `CryptProtectData`: "Typically, only a user with the same logon credential as the user who encrypted the data can decrypt the data". `CRYPTPROTECT_LOCAL_MACHINE` 을 쓰면 "Any user on the computer ... can ... decrypt" 이므로 약하다. MAC 으로 변조도 막는다 ([Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata)).
  - Windows TPM: CNG 의 Platform Crypto Provider 는 "create keys in the TPM with restrictions on their use ... without copying the keys to system memory" 하고, 내보낼 수 없게 만들 수 있다 ([Microsoft Learn, How Windows uses the TPM](https://learn.microsoft.com/en-us/windows/security/hardware-security/tpm/how-windows-uses-the-tpm)). 어떤 곡선(Ed25519 등)을 지원하는지는 확인하지 못했다.
  - macOS Keychain: 항목마다 AES-256-GCM 으로 암호화하고 metadata key 는 Secure Enclave 가 보호, access group 과 `kSecAttrAccessible...` 클래스로 접근을 제한 ([Apple Platform Security, Keychain data protection](https://support.apple.com/guide/security/keychain-data-protection-secb0694df1a/web)).
  - Rust `keyring` crate (MIT 또는 Apache-2.0, 4.2.0, 2026-08-29): `v1` feature 로 "platform-independent setting and reading of passwords/secrets on macOS, Windows, and *nix" 를 제공한다. 다만 4.x 문서는 어떤 저장소를 쓸지 직접 정하려는 앱에는 이 crate 를 link 하지 말라고 안내한다 ([docs.rs keyring 4.2.0](https://docs.rs/keyring/4.2.0/keyring/)).

### 8.4 추측 공격 제한, 동의 창, 연결 표시, 기록

- **실패 횟수 제한**
  - TeamViewer: 실패할 때마다 대기 시간을 지수적으로 늘려 "it takes 17 hours for 24 failed attempts", botnet 처럼 여러 컴퓨터가 한 대상을 노리는 경우도 막는다고 한다 ([TeamViewer Security Statement](https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/security/security-statement/)).
  - RustDesk: IP 별, IPv6 prefix 별 로그인 실패 기록(`LOGIN_FAILURES`)과 "Too many wrong attempts", 연속 실패 시 임시 비밀번호 교체 (`src/server/connection.rs`, [RustDesk repo](https://github.com/rustdesk/rustdesk)).
  - PAKE 는 온라인 추측만 가능하게 만들 뿐이라 실패 횟수 제한은 여전히 필요하다. RFC 8125 도 시도 횟수 제한과 DoS 사이 균형을 언급한다 ([RFC 8125 §1](https://www.rfc-editor.org/rfc/rfc8125#section-1)).
- **host 동의 창**: AnyDesk 는 접속 허락 창(Accept Window)에 상대 fingerprint 를 보여 준다 ([AnyDesk Fingerprint](https://support.anydesk.com/docs/fingerprint)).
- **연결 중 표시**: TeamViewer 는 연결되면 항상 작은 control panel 을 띄워 "intentionally unsuitable for covertly monitoring" 이라고 한다 ([TeamViewer Security Statement](https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/security/security-statement/)).
- **기록(audit log)**: RustDesk 코드에 연결 감사 기록(`ConnAuditPrimaryAuth`)이 있다 (`src/server/connection.rs`). TeamViewer 는 "Connection reports" 메뉴가 있다 (같은 문서의 메뉴 목록).

### 8.5 TeamViewer 와 AnyDesk 가 공개한 보안 모델 (공식 문서만)

**TeamViewer** ([Security Statement, Last Modified 2026-05-29](https://www.teamviewer.com/en/global/support/knowledge-base/teamviewer-remote/security/security-statement/))
- 연결: master server 로 handshake 후 70% 는 UDP 또는 TCP 직접 연결, 나머지는 router network 를 통한 TCP 또는 HTTP tunneling. "you don't have to open any ports".
- 암호화: 15.73 이상은 "mutually authenticated TLS 1.3", PFS, AES-256-GCM. "both communicating clients verify each other's identity using certificates issued by the TeamViewer master cluster". 이전 방식은 RSA 4096 키 교환 + AES-256. "Private keys never leave the client device ... neither TeamViewer routing servers nor any intermediary systems can decrypt".
- 비밀번호: SRP-6, "no password equivalent data is shared", 로컬에는 verifier 만 저장.
- 그 밖: 지수적 대기 brute-force 방어, ID/계정 allowlist/blocklist, 계정 2FA(TOTP), Trusted Devices, DigiCert code signing, ISO 27001 데이터센터.
- **분석 (추정)**: 인증서를 TeamViewer master cluster 가 발급하므로 E2E 의 신뢰 기준점은 TeamViewer 자신이다. 운영자가 가짜 인증서를 발급하면 MITM 이 구조적으로 가능하고, 공개 문서에는 사용자가 이를 확인할 수단(fingerprint 비교 등)이 보이지 않는다.

**AnyDesk**
- "All AnyDesk sessions are secured using TLS 1.2 with AEAD encryption". 연결 방식, 암호화 모드, client 검증 상태, fingerprint 를 toolbar 에서 볼 수 있다. salted password hashing, unattended access 2FA, ACL, on-premises 제품 ([Security tips](https://support.anydesk.com/docs/security-tips)).
- 첫 실행 때 RSA 인증서와 개인키 생성, 서버가 ID 와 공개키(fingerprint)를 영구 연결. "The AnyDesk-ID is only ever linked to one fingerprint. If you notice an AnyDesk-ID with a different fingerprint, something is wrong." ([Fingerprint](https://support.anydesk.com/docs/fingerprint)).
- "RSA 2048 asymmetric key exchange", DHE 로 PFS 라는 문구는 공식 페이지 [anydesk.com/en/security](https://anydesk.com/en/security) 에 있다고 검색 결과에 나오지만 이 페이지는 직접 열리지 않았다(HTTP 403). **미확인(2차 출처)**.
- **분석 (추정)**: TeamViewer 와 같이 ID 와 키의 연결을 회사 서버가 보증한다. 사용자가 fingerprint 를 눈으로 비교할 수 있게 노출한 점이 다르다.

---

## 9. 오픈소스 참고 구현

### 9.1 RustDesk

**기본 정보 (2026-09-23 확인)**
- license: client [rustdesk/rustdesk](https://github.com/rustdesk/rustdesk) 와 server [rustdesk/rustdesk-server](https://github.com/rustdesk/rustdesk-server) 모두 **AGPL-3.0**.
- 활동: client 최신 1.4.9 (2026-07-06), nightly 2026-07-10, 마지막 push 2026-09-23, star 약 12.4만. server 최신 1.1.16 (2026-07-20), 이전 1.1.15 는 2026-01-12.
- 소개: "You can use our rendezvous/relay server, set up your own, or write your own rendezvous/relay server" ([README](https://github.com/rustdesk/rustdesk)).

**구조: hbbs 와 hbbr** (이름과 역할을 repo 에서 확인)
- 빌드하면 세 실행 파일이 나온다: "hbbs - RustDesk ID/Rendezvous server", "hbbr - RustDesk relay server", "rustdesk-utils - RustDesk CLI utilities" ([rustdesk-server README](https://github.com/rustdesk/rustdesk-server)).
- port: hbbs 는 TCP 21114-21116, 21118 과 UDP 21116, hbbr 는 TCP 21117, 21119. 21118/21119 는 web client 용 WebSocket ([self-host docs](https://rustdesk.com/docs/en/self-host/)).
- `ALWAYS_USE_RELAY=Y` 로 직접 연결을 끌 수 있다 ([rustdesk-server README](https://github.com/rustdesk/rustdesk-server)). 반대로 "relay 금지" 설정은 문서에서 찾지 못했다.
- 더 단순한 출발점으로 [rustdesk-server-demo](https://github.com/rustdesk/rustdesk-server-demo) 를 권한다 (README).
- 무료 OSS 와 유료 Pro 가 있고 Pro 는 web console, OIDC, LDAP, 2FA, 여러 relay 등을 더한다 ([self-host docs](https://rustdesk.com/docs/en/self-host/)).

**NAT traversal 과 fallback**
- 순서: hole punching 으로 직접 연결 시도, 실패하면 relay ([self-host docs](https://rustdesk.com/docs/en/self-host/)).
- 코드상 경로: UDP punch, TCP punch, IPv6 punch, LAN listen, 그리고 rendezvous 를 거친 WebRTC ICE 경로 (`src/rendezvous_mediator.rs` 주석). WebRTC 는 webrtc-rs 를 fork 한 `rustdesk-org/webrtc` 를 특정 commit 으로 고정해 쓰고, KCP(`kcp-sys`)도 쓴다 (`Cargo.toml`). `Cargo.toml` 주석에 따르면 upstream v0.13 에서 (1) Windows 에서 IPv6 host candidate 를 하나도 모으지 못하는 버그, (2) SCTP 의 1초 RTO 하한 때문에 RTT 24~64 ms 링크에서도 손실 한 번에 1~3초가 걸리는 문제를 fork 에서 고쳤고, data channel 은 기본으로 congestion window 없이 KCP 처럼 보낸다.

**암호화 방식** (`src/client.rs`, `src/common.rs`, [hbb_common `src/tcp.rs`](https://github.com/rustdesk/hbb_common))
- libsodium 의 Rust binding `sodiumoxide` 를 쓴다. 각 peer 는 장기 Ed25519 서명 키를 가진다. hbbs 가 (ID, 공개키) 를 서버 key 로 서명해 주고, client 는 설정된 서버 공개키(기본 `RS_PUB_KEY` 또는 사용자가 넣은 key)로 이를 확인한다. 그다음 host 가 서명한 임시 공개키를 받아, client 가 대칭키를 만들어 `box`(Curve25519 + XSalsa20-Poly1305)로 봉해 보내고, 이후 stream 은 `secretbox`(XSalsa20-Poly1305)로 암호화한다. kx_version 1 은 key exchange 내용(transcript)으로 방향별 subkey 를 만든다.
- **신뢰 기준점은 rendezvous 서버 key** 다. 공개 서버를 쓰면 RustDesk 운영자, 자체 서버를 쓰면 그 서버 관리자다.
- **주의: 조건이 안 맞으면 암호화 없이 진행한다.** 서명된 공개키가 없거나 검증에 실패하면 "Fall through like TCP's non-secure path", 공개키가 안 맞으면 "pk mismatch, fall back to non-secure" (`src/client.rs` `secure_connection`). 주석은 이유로 "bailing would only move the session to a relay that fails the same check and then runs in plaintext"를 든다. WebRTC 경로만은 이 경우 끊는다.
- `sodiumoxide` 는 archived 상태이고 README 첫 줄이 "[DEPRECATED]"다 ([sodiumoxide](https://github.com/sodiumoxide/sodiumoxide)).

**비밀번호 인증**
- host 가 salt 와 challenge 를 보내고, client 는 `SHA256(SHA256(password + salt) + challenge)` 를 보낸다 (`src/client.rs`, `src/server/connection.rs` `verify_h1`). PAKE 가 아니다. 위의 "암호화 없이 진행" 경로에서 이 값을 본 공격자는 짧은 임시 비밀번호를 오프라인으로 대입할 수 있다 (**추정**).
- 실패 제한: IP 와 IPv6 prefix 단위 기록, 연속 실패 시 임시 비밀번호 교체 (8.4절).

**영상과 플랫폼**
- codec: VP8, VP9, AV1, H264, H265 (`libs/scrap/src/common/mod.rs` `CodecFormat`). 하드웨어 codec 모듈(`hwcodec.rs`, `vram.rs`, `mediacodec.rs`)이 있다.
- 화면 캡처 모듈: Windows `dxgi.rs`, macOS `quartz.rs`, Linux `x11.rs`/`wayland.rs`, Android `android.rs` (`libs/scrap/src/common/`).
- GUI 는 Flutter (Sciter 는 deprecated) ([README](https://github.com/rustdesk/rustdesk)).

**배울 점과 license 영향**
- 배울 점: ID 서버가 peer 공개키를 서명해 주는 구조, WebRTC DTLS fingerprint 를 peer 신원 키 서명에 묶는 방식, IPv6 prefix 단위 실패 제한, 여러 punch 경로를 동시에 시도하는 구조.
- 피할 점: 검증 실패 시 암호화 없이 진행, hash challenge 비밀번호 인증, deprecated 암호 라이브러리.
- license: AGPL-3.0 코드를 가져다 쓰면 결과물 전체를 AGPL-3.0 으로 공개해야 한다 (**추정**, copyleft 일반 해석). 추가로 §13 은 수정한 프로그램을 네트워크로 쓰게 하면 "an opportunity to receive the Corresponding Source"를 제공하라고 요구한다 ([AGPL-3.0 §13](https://www.gnu.org/licenses/agpl-3.0.txt)). 즉 수정한 hbbs 를 운영해도 소스 공개 의무가 생긴다. 코드를 읽고 설계만 참고하는 것은 이 의무와 무관하다 (**추정**, 법률 자문 아님).

### 9.2 그 밖의 참고 프로젝트

이 절의 GitHub 링크 일부는 조사 시점 commit SHA 로 고정했다.

**Sunshine + Moonlight** (Sunshine GPL-3.0, 최신 안정판 v2026.914.233613 (2026-09-15). moonlight-qt GPL-3.0, v6.1.0 (2024-09-17))
- 저지연 game streaming 의 host(Sunshine)와 client(Moonlight). Windows 캡처는 DXGI Desktop Duplication / Windows.Graphics.Capture, 인코더는 NVENC/AMF/QuickSync/Media Foundation/Software, macOS 는 ScreenCaptureKit + VideoToolbox ([Sunshine README](https://github.com/LizardByte/Sunshine#readme)). codec 은 H.264, HEVC, AV1 ([moonlight-common-c Limelight.h](https://github.com/moonlight-stream/moonlight-common-c/blob/62e066388f1a1b133e0bee947b9a374311a3354b/src/Limelight.h#L225-L234)). input 은 ENet(UDP 위 신뢰성 계층) control stream 으로 보낸다.
- pairing 은 **PAKE 가 아니다**. PIN 은 숫자 4자리만 받고 ([Sunshine nvhttp.cpp](https://github.com/LizardByte/Sunshine/blob/c48e50e418b27cba2b387c3a1ae9605da8a96341/src/nvhttp.cpp#L632-L636)), `SHA-256(salt || PIN)` 앞 16 byte 를 AES key 로 써서 평문 HTTP 위에서 challenge 를 주고받아 서로의 인증서를 pin 한다. 이후 세션 키는 pin 한 인증서의 mutual TLS 로 전달한다 ([moonlight-qt nvpairingmanager.cpp](https://github.com/moonlight-stream/moonlight-qt/blob/032529d782242e3833e0b3b147dbbf96e878e3ca/app/backend/nvpairingmanager.cpp#L227-L263), [Sunshine nvhttp.cpp](https://github.com/LizardByte/Sunshine/blob/c48e50e418b27cba2b387c3a1ae9605da8a96341/src/nvhttp.cpp#L729-L881)). pairing 과정을 엿본 공격자는 4자리 PIN 을 오프라인으로 알아낼 수 있다 (**추정**).
- 영상 암호화는 선택이다. LAN 기본값 "never", WAN 기본값 "opportunistic" ([Sunshine config.cpp](https://github.com/LizardByte/Sunshine/blob/c48e50e418b27cba2b387c3a1ae9605da8a96341/src/config.cpp#L821-L822)). control stream 은 항상 AES-GCM.
- NAT: hole punching 과 relay 가 없다. 대신 Internet Hosting Tool(GPL-3.0, v5.6.1-r2, 2023-08-26)이 "Supports all major port forwarding protocols (UPnP, NAT-PMP, and PCP)", "Supports forwarding on Carrier-Grade NATs (CGNs) by directly using PCP" ([Internet-Hosting-Tool](https://github.com/moonlight-stream/Internet-Hosting-Tool)). port: TCP 47984, 47989, 48010, UDP 47998-48000 등 ([Moonlight Setup Guide](https://github.com/moonlight-stream/moonlight-docs/wiki/Setup-Guide)).
- 우리 설계에 주는 것: Windows/macOS 캡처와 인코딩 경로 선택의 검증된 참고. host 쪽 port mapping 을 여러 NAT 층에 시도하는 방식. GPL-3.0 이라 코드를 가져오면 제품 전체가 GPL-3.0 이 된다.

**MeshCentral** (Apache-2.0, 1.2.5, 2026-08-12)
- agent 가 서버에 HTTPS(443) WebSocket 으로 **먼저 밖으로 접속**하는 구조라 host 쪽에 inbound port 가 필요 없다. agent 는 CA 대신 서버 인증서 hash 를 pin 한다 ([MeshCentral design doc](https://github.com/Ylianst/MeshCentralDocs/blob/175bd538ab9382ac047df1790c435f9ebf8625f0/docs/design/index.md?plain=1#L308-L342)).
- 기본은 서버 relay 이고 "the session is not end-to-end encrypted. The server is performing a TLS decrypt and re-encrypt" ([design doc](https://github.com/Ylianst/MeshCentralDocs/blob/175bd538ab9382ac047df1790c435f9ebf8625f0/docs/design/index.md?plain=1#L348)). relay 로 연결한 뒤 가능하면 WebRTC data channel 로 넘어가지만 설정 `webRTC` 기본값은 false ([config schema](https://github.com/Ylianst/MeshCentral/blob/c146225fdd674fb60ddfbb2406382c12ad65ee9c/meshcentral-config-schema.json#L493)).
- 화면은 video codec 대신 변경된 tile 을 JPEG 로 보낸다 ([MeshAgent tile.cpp](https://github.com/Ylianst/MeshAgent/blob/97a0582c2f0f855743c76ffea477e32bd2677c67/meshcore/KVM/Windows/tile.cpp#L192-L204)).
- 우리 설계에 주는 것: "host 가 항상 밖으로 먼저 접속"이라는 방향성, 서버 인증서 hash pinning. 서버 relay 구조 자체는 제약 1 과 충돌.

**Apache Guacamole** (Apache-2.0, 1.6.0, 2025-06-22, [releases](https://guacamole.apache.org/releases/))
- "clientless remote desktop gateway" 로 VNC, RDP, SSH 를 지원 ([guacamole.apache.org](https://guacamole.apache.org/)). gateway 인 guacd 가 원격 데스크톱 protocol 과 Guacamole protocol 사이를 번역하므로 **gateway 가 화면과 입력을 모두 평문으로 본다** ([architecture](https://guacamole.apache.org/doc/gug/guacamole-architecture.html)).
- 중앙 gateway 구조라 제약 1 과 정면으로 충돌. browser client UX 정도만 참고.

**noVNC, VNC/RFB** (noVNC core MPL-2.0, v1.7.0, 2026-04-28, [LICENSE.txt](https://github.com/novnc/noVNC/blob/master/LICENSE.txt))
- [RFC 6143](https://www.rfc-editor.org/rfc/rfc6143) 이 정의한 security type 은 None 과 VNC Authentication 두 가지뿐이다. VNC auth 는 DES challenge-response 이고 비밀번호가 8자로 잘리며 "known to be cryptographically weak" ([§7.2.2](https://www.rfc-editor.org/rfc/rfc6143#section-7.2.2)). 보안 절은 "no protection against observation of or tampering with the data stream"이라며 IPsec, SSH 등을 권한다 ([§9](https://www.rfc-editor.org/rfc/rfc6143#section-9)).
- **reverse connection 이 RFC 에 적혀 있다**: "In some cases, the initial roles of the client and server are reversed, with the RFB client listening on port 5500, and the RFB server contacting the RFB client" ([§2](https://www.rfc-editor.org/rfc/rfc6143#section-2)). 6장의 선례.
- noVNC 는 browser 용 VNC client 이고 WebSocket 을 모르는 VNC 서버에는 websockify(LGPL-3.0) proxy 가 필요하다 ([noVNC README](https://github.com/novnc/noVNC#server-requirements)).

**Parsec** (독점, 공식 문서만)
- 연결 순서: Parsec 의 signaling(WebSocket API)과 STUN(UDP 3478)으로 공인 주소를 교환한 뒤 "they both attempt to connect to each another at the same time", 기본으로 UPnP 도 쓴다 ([Components and Connection Sequence](https://support.parsec.app/hc/en-us/articles/32361410290324-Components-and-Connection-Sequence)).
- 연결 조건에 "Host and client must not be inside a 'Double NAT' or CGNAT network", "SSL traffic must not be decrypted using SSL inspection" 이 있다 ([Parsec Connectivity Requirements, 2026-09-06 갱신](https://support.parsec.app/hc/en-us/articles/32381460716180-Parsec-Connectivity-Requirements)). relay(High Performance Relay)는 Teams Enterprise license 에서 고객이 직접 운영 ([HPR overview](https://support.parsec.app/hc/en-us/articles/32381058430740-High-Performance-Relay-HPR-Server-Overview-and-Prerequisites)).
- 전송은 UDP 기반 독자 protocol BUD, "encrypted with DTLS 1.2", TCP 같은 신뢰성과 자체 congestion control. WebRTC 도 시도했지만 원하는 성능이 안 나왔다고 한다 ([Parsec blog](https://parsec.app/blog/a-networking-protocol-built-for-the-lowest-latency-interactive-game-streaming-1fd5a03a6007), 날짜 표시 없음).
- 보안: "All peer-to-peer audio/video/input data is encrypted via DTLS 1.2 (AES128)", 사용자별 인증서를 Parsec backend 가 검증 ([Security At Parsec](https://support.parsec.app/hc/en-us/articles/32361366289940-Security-At-Parsec)). 신뢰 기준점은 Parsec 계정 backend.
- host: Windows 10 이상, macOS 10.15 이상. codec H.264/H.265 ([Feature Matrix](https://support.parsec.app/hc/en-us/articles/32381463419924-Feature-Matrix)).
- 우리 설계에 주는 것: relay 없는 구성(signaling + STUN + 동시 UDP + UPnP)이 상용 제품의 기본 경로이고, 그 제품도 CGNAT/double NAT 에서는 relay 없이 안 된다고 문서로 인정한다는 점.

**Chrome Remote Desktop** (참고)
- WebRTC transport 를 쓰고, 정책 `RemoteAccessHostAllowRelayedConnection` 기본값 true 로 "relay servers ... when a direct connection is not available" 을 허용한다 ([Chromium policy definitions](https://chromium.googlesource.com/chromium/src/+/main/components/policy/resources/templates/policy_definitions/RemoteAccess/)). 즉 기본으로 Google relay 를 fallback 으로 쓴다.

**라이선스 영향 요약**
- GPL-3.0 (Sunshine, Moonlight, moonlight-common-c, Internet Hosting Tool), AGPL-3.0 (RustDesk): 코드를 가져오거나 link 하면 제품 전체를 같은 license 로 공개. 아이디어와 protocol 을 새로 구현하는 것은 자유.
- Apache-2.0 (MeshCentral, MeshAgent, Guacamole): 가져다 써도 됨. NOTICE 유지, 특허 허락 조항.
- MPL-2.0 (noVNC core, libdatachannel): 수정한 파일만 공개.
- 위 해석은 **추정**이며 법률 자문이 아니다.

---

## 10. "data relay 없음" 설계의 결론

아래 비율은 4장의 측정과 vendor 수치를 조합한 **추정**이다. 우리 사용자 집단(가족, 주로 가정 망)을 직접 잰 자료는 없다.

| 구성 (앞 단계에 차례로 더함) | 연결 실패 예상 | 무엇이 살아나나 | 근거 |
|---|---|---|---|
| ① UDP hole punching 만 (signaling + STUN) | 약 10~30% | 양쪽 EIM 인 대부분의 가정 | 4장: libp2p 70% ± 7.1%, TeamViewer 직접 연결 70%, Tailscale/iroh 추정 약 90% |
| ② ① + birthday port 예측 + TCP simultaneous open | ①보다 줄지만 수치 없음 | 한쪽만 EDM 인 경우(휴대폰 망 한쪽 등), UDP 만 막힌 망 일부 | 3.3, 3.4절. 양쪽 EDM(양쪽 다 휴대폰 CGN 등)은 여전히 불가 |
| ③ ② + host 쪽 UPnP/NAT-PMP/PCP | 더 줄어듦, 수치 없음 | host 공유기가 응답하는 경우 host 쪽 NAT 종류와 무관하게 성공. 2011년 기준 가정의 약 35% 가 UPnP 응답 | 3.1절 |
| ④ ③ + viewer reverse 연결 (viewer 집, 공인 IPv4 또는 IPv6 inbound, TCP 443 + UDP) | host 가 가정 망이면 **거의 0 에 가까움** | host 쪽 NAT, CGNAT, 휴대폰 망, UDP 차단과 상관없이 host 가 밖으로 나가기만 하면 됨 | 가정 NAT 는 outbound 를 기본 허용 ([Tailscale blog](https://tailscale.com/blog/how-nat-traversal-works)), ISP 전체가 UDP 를 막는 사례는 관찰되지 않음 ([QUIC 논문 §7.2](https://static.googleusercontent.com/media/research.google.com/en//pubs/archive/46403.pdf)) |

**④ 에서도 남는 실패**
- viewer 가 집 밖에 있거나 viewer 의 ISP 가 CGNAT 이면 ④ 가 없어지고 ①~③ 만 남는다. 이때 실패율은 다시 10~30% 쪽.
- host 가 기업망에 있고 TLS 검사 proxy 가 있으면, 공개 인증서를 쓴 TLS/WebSocket 경로만 통과한다 (6.2절).
- viewer 의 공인 IP 가 막 바뀌어 DDNS 가 갱신되기 전 (TTL 최소 60초, 6.3절).

**사용자가 보게 될 것 (설계 제안, 추정)**
- relay 가 없으므로 "느리지만 연결은 됨" 상태가 존재하지 않는다. 결과는 "직접 연결 성공" 또는 "연결 실패" 둘뿐이다.
- 실패 시 원인 범주를 보여 주고 사람이 할 수 있는 조치를 안내해야 한다. 예: "viewer 가 집 네트워크에 있지 않습니다", "상대가 휴대폰 hotspot 에 연결되어 있습니다. 집 Wi-Fi 로 바꿔 주세요", "회사 네트워크가 연결을 막고 있습니다".
- 가족 지원 시나리오(viewer 는 대개 집, host 는 가족의 집)에서는 ④ 가 주 경로가 되고 ①~③ 은 viewer 가 외출했을 때의 보조 경로가 된다.

| 기법 | 누가 설정 | 회복하는 실패 |
|---|---|---|
| viewer 공유기 port forwarding + DDNS (경로 A/B) | viewer 가 한 번 | host 쪽 원인 대부분 (CGNAT, EDM, 휴대폰 망) |
| viewer IPv6 inbound 규칙 | viewer 가 한 번 | host 가 IPv6 일 때 위와 같음. 한국 host 는 17% 정도만 여기에 들어감 |
| host 쪽 UPnP/PCP 자동 mapping | 프로그램이 자동 | viewer 외출 중이고 host 공유기가 응답할 때 |
| birthday port 예측 | 프로그램이 자동 | 한쪽만 EDM |
| TCP simultaneous open | 프로그램이 자동 | UDP 만 막힌 망 |
| TLS/WebSocket over 443 + 공개 인증서 | viewer 가 한 번 (인증서 자동화) | 기업망, TLS 검사 proxy |

---

## 11. 아키텍처 선택지 비교

제약 1(data relay 없음)을 만족하는 세 가지. 결정은 하지 않는다. 세 선택지 모두 "viewer reverse 연결 + 경로 경주 + PAKE 로 채널 묶기"를 공통으로 전제한다. 다른 점은 전송 stack 이다.

| | 1. WebRTC stack (ICE, TURN 없음) | 2. QUIC 기반 (iroh relay 끔, 또는 quinn/noq 직접) | 3. RustDesk fork (hbbs 만, hbbr 없음) |
|---|---|---|---|
| 필요한 서버 | signaling: Cloudflare Workers + DO 무료 (7.1절). STUN: `stun.cloudflare.com` | signaling: Workers + DO (viewer DDNS 만 쓸 때는 생략 가능). STUN 이 필요하면 공개 STUN 을 직접 호출 | hbbs (TCP 21115-21116, UDP 21116 등). 임의 TCP/UDP port 라 Workers 불가, VM 필요 (Oracle Always Free, idle 회수 위험) |
| 직접 연결 기대치 | ICE 가 host/srflx/IPv6/ICE-TCP 후보를 한 번에 경주. viewer 의 port forwarding 주소를 후보로 넣으면 경로 A/B/D/F 포함 (Pion `SetICEAddressRewriteRules`, str0m `add_local_candidate`). host 쪽 UPnP 는 별도 라이브러리로 얻어 후보에 추가 | reverse 경로(A/B)는 바로 됨 (`EndpointAddr` 에 viewer 주소). **둘 다 NAT 뒤일 때의 hole punching 은 iroh 가 relay 없이는 안 하므로 직접 구현**. TCP 경로도 직접 구현 (iroh custom transport 는 불안정 API) | UDP/TCP/IPv6 punch, WebRTC ICE 경로가 이미 있음. hbbr 가 없으면 punch 실패가 곧 연결 실패. reverse 연결 모드는 새로 만들어야 함 (6.1절: 현재 없음) |
| 기업망 TLS 검사 proxy 통과 | ICE-TCP 는 TLS 가 아니라서 App-ID 에 unknown-tcp 로 막힐 가능성 (**추정**). 별도 WebSocket/TLS 경로 필요 | 별도 WebSocket/TLS 경로 필요 | 별도 구현 필요 |
| E2E 보안 | DTLS. signaling 이 fingerprint 를 바꿔치기할 수 있으므로 PAKE 결과로 fingerprint 를 묶어야 함 (8.1절) | TLS 1.3 + Ed25519 EndpointId. EndpointId 도 PAKE 나 pairing 으로 인증해야 함 | NaCl box/secretbox, 신뢰 기준점은 hbbs key. **검증 실패 시 암호화 없이 진행하는 경로를 제거**해야 하고, hash challenge 비밀번호를 PAKE 로 바꿔야 함 (9.1절) |
| 구현 부담 | 중간. 영상 전송(RTP, congestion control, NACK)은 표준에 있음. libwebrtc 는 codec 까지 있지만 빌드가 무겁고, Pion/str0m/webrtc-rs 는 캡처·인코딩을 따로 붙여야 함 | 중간~큼. 영상용 congestion control, jitter 처리, 손실 대응을 QUIC stream/datagram 위에 직접 만들어야 함 | 기능(codec, 클립보드, 파일, 멀티 모니터)은 가장 빨리 확보. 대신 큰 코드베이스를 깊이 고쳐야 하고 upstream 추적 부담 |
| 위험 | webrtc-rs 는 1.0 전이라 API 변경 가능. RustDesk 는 webrtc-rs v0.13 의 Windows IPv6 candidate 버그와 SCTP 지연 문제를 fork 로 고쳐 썼다 (RustDesk `Cargo.toml` 주석, 0.21 에서 해결됐는지는 미확인). str0m 은 P2P 용도로 덜 테스트됨 | relay 없는 사용은 iroh 의 주 경로가 아님 (**추정**, 테스트가 적을 것). noq 는 2026-03 에 분리된 새 hard fork | **AGPL-3.0** (제품 전체 공개 의무, 9.1절). `sodiumoxide` deprecated. hbbs 를 VM 에 상시 운영 |
| host 언어 | Go (Pion), Rust (str0m, webrtc-rs), C++ (libwebrtc, libdatachannel) | Rust | Rust + Flutter |

**선택지에서 뺀 것 (한 줄씩)**
- TURN, iroh public/자체 relay 의 data 전달, RustDesk hbbr, Tailscale DERP: 세션 data 를 중계하므로 제약 1 위반.
- 회색지대(사용자 판단 필요): data 한도를 아주 작게 건 "좌표 전용 relay". libp2p circuit relay v2 기본 한도(2분, 방향당 128 KiB), 또는 `ClientRateLimit` 을 낮게 건 자체 iroh-relay. data 비용은 거의 0 이지만 relay process 는 존재한다 (5.3절).
