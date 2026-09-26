mod error;
mod link;
mod net;
mod peer;

#[cfg(any(test, feature = "test-support"))]
pub mod fake_signal;

pub use error::TransportError;
pub use link::{Link, LinkError, WsLink};
pub use net::local_ips;
pub use peer::{PeerEvent, Peer, PendingOffer};
