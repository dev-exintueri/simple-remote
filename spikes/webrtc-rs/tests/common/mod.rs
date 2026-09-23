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
