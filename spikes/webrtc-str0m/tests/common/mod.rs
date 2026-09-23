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
