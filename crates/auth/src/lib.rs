mod code;
mod error;
mod identity;
mod pake;
mod permit;
mod reconnect;
mod rng;
mod sealed;
mod store;

pub use code::OneTimeCode;
pub use error::AuthError;
pub use identity::{DeviceKeys, PeerIdentity, Role, key_fingerprint, verify_hello};
pub use pake::{HostPake, SessionKeys, ViewerPake};
pub use permit::{
    HostBook, HostPermit, PERMIT_TTL, PermitError, PermitState, ViewerBook, ViewerPermit, unix_ms,
};
pub use reconnect::{HostReconnect, NoiseChannel, ReconnectSession, ViewerReconnect};
pub use rng::rng;
pub use sealed::{SealedReceiver, SealedSender};
#[cfg(any(test, feature = "insecure-dev-store"))]
pub use store::DevPlaintextProtector;
pub use store::{Protector, StateFile, StoreError};
