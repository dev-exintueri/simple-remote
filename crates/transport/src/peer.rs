use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::thread::sleep;
use std::time::{Duration, Instant};

use str0m::change::{SdpAnswer, SdpOffer, SdpPendingOffer};
use str0m::channel::{ChannelConfig, ChannelId, Reliability};
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, Event, Input, Output, Rtc};

use crate::error::TransportError;

/// `Peer::create_offer` 이 돌려주는, 아직 답이 오지 않은 offer 상태.
/// `str0m::change::SdpPendingOffer` 를 감싼 newtype.
pub struct PendingOffer(SdpPendingOffer);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerEvent {
    Connected,
    ChannelOpen,
    Data(Vec<u8>),
    Closed,
}

const CONTROL_LABEL: &str = "control";

pub struct Peer {
    rtc: Rtc,
    sockets: HashMap<SocketAddr, UdpSocket>,
    control: Option<ChannelId>,
}

impl Peer {
    /// `bind_ips` 마다 UDP socket 을 `ip:0` 에 nonblocking 으로 열고,
    /// 각각을 host candidate 로 등록한다. `bind_ips` 가 비어 있으면 오류.
    pub fn new(bind_ips: &[IpAddr]) -> Result<Peer, TransportError> {
        if bind_ips.is_empty() {
            return Err(TransportError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "bind_ips must not be empty",
            )));
        }

        let mut rtc = Rtc::builder().build(Instant::now());
        let mut sockets = HashMap::new();

        for ip in bind_ips {
            let sock = UdpSocket::bind(SocketAddr::new(*ip, 0))?;
            sock.set_nonblocking(true)?;
            let addr = sock.local_addr()?;
            let candidate = Candidate::host(addr, "udp")
                .map_err(|e| TransportError::Rtc(e.to_string()))?;
            rtc.add_local_candidate(candidate);
            sockets.insert(addr, sock);
        }

        Ok(Peer { rtc, sockets, control: None })
    }

    /// control data channel 을 추가하고 SDP offer 를 만든다.
    pub fn create_offer(&mut self) -> Result<(String, PendingOffer), TransportError> {
        let mut api = self.rtc.sdp_api();
        api.add_channel_with_config(ChannelConfig {
            label: CONTROL_LABEL.into(),
            ordered: true,
            reliability: Reliability::Reliable,
            ..Default::default()
        });
        let (offer, pending) = api
            .apply()
            .expect("add_channel_with_config always requires negotiation for the first channel");
        Ok((offer.to_sdp_string(), PendingOffer(pending)))
    }

    /// 원격에서 온 offer 를 받아들이고 answer SDP 를 만든다.
    pub fn accept_offer(&mut self, sdp: &str) -> Result<String, TransportError> {
        let offer = SdpOffer::from_sdp_string(sdp).map_err(|e| TransportError::Rtc(e.to_string()))?;
        let answer = self
            .rtc
            .sdp_api()
            .accept_offer(offer)
            .map_err(|e| TransportError::Rtc(e.to_string()))?;
        Ok(answer.to_sdp_string())
    }

    /// 원격에서 온 answer 를 받아들여 협상을 마친다.
    pub fn accept_answer(&mut self, pending: PendingOffer, sdp: &str) -> Result<(), TransportError> {
        let answer = SdpAnswer::from_sdp_string(sdp).map_err(|e| TransportError::Rtc(e.to_string()))?;
        self.rtc
            .sdp_api()
            .accept_answer(pending.0, answer)
            .map_err(|e| TransportError::Rtc(e.to_string()))
    }

    /// `max_wait` 가 지나거나 event 가 하나라도 생기면 돌아온다.
    pub fn poll(&mut self, max_wait: Duration) -> Result<Vec<PeerEvent>, TransportError> {
        let end = Instant::now() + max_wait;
        let mut events = Vec::new();

        loop {
            self.drain_output(&mut events)?;

            if !events.is_empty() || Instant::now() >= end {
                return Ok(events);
            }

            let got_input = self.pump_sockets()?;

            self.rtc
                .handle_input(Input::Timeout(Instant::now()))
                .map_err(|e| TransportError::Rtc(e.to_string()))?;

            if !got_input {
                sleep(Duration::from_millis(1));
            }
        }
    }

    /// `poll_output` 을 `Output::Timeout` 이 나올 때까지 돌리며 `Transmit` 은 해당 socket 으로 보내고
    /// `Event` 는 `PeerEvent` 로 변환해 모은다.
    fn drain_output(&mut self, events: &mut Vec<PeerEvent>) -> Result<(), TransportError> {
        loop {
            let output = self
                .rtc
                .poll_output()
                .map_err(|e| TransportError::Rtc(e.to_string()))?;

            match output {
                Output::Timeout(_) => return Ok(()),
                Output::Transmit(t) => {
                    if let Some(sock) = self.sockets.get(&t.source) {
                        // UDP is best effort: a failed send (no route to one candidate,
                        // full buffer) only loses this datagram. ICE/SCTP retransmit.
                        let _ = sock.send_to(&t.contents, t.destination);
                    }
                }
                Output::Event(event) => {
                    if let Some(e) = self.map_event(event) {
                        events.push(e);
                    }
                }
            }
        }
    }

    /// 모든 socket 에서 nonblocking 으로 한 datagram 씩 읽어 `Rtc` 에 먹인다.
    /// 무엇이라도 읽었으면 `true`.
    fn pump_sockets(&mut self) -> Result<bool, TransportError> {
        let mut buf = [0u8; 2000];
        let mut got_input = false;

        for (&local_addr, sock) in self.sockets.iter() {
            match sock.recv_from(&mut buf) {
                Ok((n, source)) => {
                    got_input = true;
                    let input = Input::Receive(
                        Instant::now(),
                        Receive {
                            proto: Protocol::Udp,
                            source,
                            destination: local_addr,
                            contents: buf[..n]
                                .try_into()
                                .map_err(|_| TransportError::Rtc("datagram too large".into()))?,
                        },
                    );
                    self.rtc
                        .handle_input(input)
                        .map_err(|e| TransportError::Rtc(e.to_string()))?;
                }
                Err(e) if recv_error_is_transient(e.kind()) => {}
                Err(e) => return Err(TransportError::Io(e)),
            }
        }

        Ok(got_input)
    }

    fn map_event(&mut self, event: Event) -> Option<PeerEvent> {
        match event {
            Event::Connected => Some(PeerEvent::Connected),
            Event::ChannelOpen(id, label) => {
                if label == CONTROL_LABEL {
                    self.control = Some(id);
                    Some(PeerEvent::ChannelOpen)
                } else {
                    None
                }
            }
            Event::ChannelData(data) => Some(PeerEvent::Data(data.data)),
            Event::ChannelClose(id) => {
                if self.control == Some(id) {
                    Some(PeerEvent::Closed)
                } else {
                    None
                }
            }
            Event::Closed => Some(PeerEvent::Closed),
            _ => None,
        }
    }

    /// control channel 로 데이터를 보낸다. 채널이 아직 열리지 않았으면 `NoChannel`.
    pub fn send(&mut self, data: &[u8]) -> Result<(), TransportError> {
        let id = self.control.ok_or(TransportError::NoChannel)?;
        let mut channel = self.rtc.channel(id).ok_or(TransportError::NoChannel)?;
        let wrote = channel
            .write(true, data)
            .map_err(|e| TransportError::Rtc(e.to_string()))?;
        if !wrote {
            return Err(TransportError::Backpressure);
        }
        Ok(())
    }

    pub fn local_fingerprint(&mut self) -> Vec<u8> {
        self.rtc.direct_api().local_dtls_fingerprint().bytes.clone()
    }

    pub fn remote_fingerprint(&mut self) -> Option<Vec<u8>> {
        self.rtc
            .direct_api()
            .remote_dtls_fingerprint()
            .map(|f| f.bytes.clone())
    }

    pub fn close(&mut self) {
        let _ = self.rtc.close();
    }
}

/// `recv_from` 오류 중 socket 을 계속 써도 되는 것.
/// - `WouldBlock` (Unix) / `TimedOut` (Windows): 읽을 datagram 이 없음.
/// - `ConnectionReset`: Windows 가 앞선 `send_to` 에 대한 ICMP port unreachable 을
///   연결하지 않은 UDP socket 에도 `WSAECONNRESET` 으로 알린다. datagram 하나를 잃은 것뿐.
/// - `ConnectionRefused`: 같은 ICMP 보고를 이 값으로 내는 stack 이 있다.
fn recv_error_is_transient(kind: std::io::ErrorKind) -> bool {
    use std::io::ErrorKind::*;
    matches!(kind, WouldBlock | TimedOut | ConnectionReset | ConnectionRefused)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::ErrorKind;

    #[test]
    fn recv_errors_that_mean_no_data_or_one_lost_datagram_are_transient() {
        // WouldBlock (Unix) / TimedOut (Windows): nothing to read.
        assert!(recv_error_is_transient(ErrorKind::WouldBlock));
        assert!(recv_error_is_transient(ErrorKind::TimedOut));
        // Windows WSAECONNRESET after an ICMP port unreachable for an earlier send_to.
        assert!(recv_error_is_transient(ErrorKind::ConnectionReset));
        // Same ICMP report surfaced as ECONNREFUSED on some stacks.
        assert!(recv_error_is_transient(ErrorKind::ConnectionRefused));
    }

    #[test]
    fn other_recv_errors_are_fatal() {
        for kind in [
            ErrorKind::PermissionDenied,
            ErrorKind::InvalidInput,
            ErrorKind::NotConnected,
            ErrorKind::Other,
        ] {
            assert!(!recv_error_is_transient(kind), "{kind:?} must propagate");
        }
    }
}
