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
