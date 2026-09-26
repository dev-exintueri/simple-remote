use hkdf::Hkdf;
use pakery_core::crypto::{CpaceGroup, Hash};
use pakery_crypto::{P256Group, Sha512Hash, Spake2P256};
use pakery_spake2::{PartyA, PartyB, Spake2Error};
use sha2::Sha256;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::AuthError;
use crate::code::OneTimeCode;
use crate::rng::rng;

const AAD: &[u8] = b"simple-remote pake v1";
const VIEWER_ID: &[u8] = b"simple-remote viewer";
const HOST_ID_PREFIX: &[u8] = b"simple-remote host ";
const CODE_CONTEXT: &[u8] = b"simple-remote code v1\0";

const INFO_VIEWER_TO_HOST: &[u8] = b"simple-remote v1 seal viewer->host";
const INFO_HOST_TO_VIEWER: &[u8] = b"simple-remote v1 seal host->viewer";
const INFO_HELLO_BINDING: &[u8] = b"simple-remote v1 hello binding";

/// 합의된 세션 key 3 종. drop 시 zeroize 된다.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct SessionKeys {
    pub viewer_to_host: [u8; 32],
    pub host_to_viewer: [u8; 32],
    pub hello_binding: [u8; 32],
}

fn host_id_bytes(host_id: &str) -> Vec<u8> {
    let mut v = Vec::with_capacity(HOST_ID_PREFIX.len() + host_id.len());
    v.extend_from_slice(HOST_ID_PREFIX);
    v.extend_from_slice(host_id.as_bytes());
    v
}

fn code_scalar(code: &OneTimeCode) -> Result<<P256Group as CpaceGroup>::Scalar, AuthError> {
    let mut input = Vec::with_capacity(CODE_CONTEXT.len() + 6);
    input.extend_from_slice(CODE_CONTEXT);
    input.extend_from_slice(code.as_str().as_bytes());
    let digest = Sha512Hash::digest(&input);
    P256Group::scalar_from_wide_bytes(&digest).map_err(|_| AuthError::Malformed("pake"))
}

fn derive_session_keys(ke: &[u8]) -> SessionKeys {
    let hk = Hkdf::<Sha256>::new(None, ke);
    let mut viewer_to_host = [0u8; 32];
    let mut host_to_viewer = [0u8; 32];
    let mut hello_binding = [0u8; 32];
    hk.expand(INFO_VIEWER_TO_HOST, &mut viewer_to_host)
        .expect("32 byte output is within HKDF-SHA256 limit");
    hk.expand(INFO_HOST_TO_VIEWER, &mut host_to_viewer)
        .expect("32 byte output is within HKDF-SHA256 limit");
    hk.expand(INFO_HELLO_BINDING, &mut hello_binding)
        .expect("32 byte output is within HKDF-SHA256 limit");
    SessionKeys {
        viewer_to_host,
        host_to_viewer,
        hello_binding,
    }
}

fn to_mac(bytes: &[u8]) -> Result<[u8; 32], AuthError> {
    bytes.try_into().map_err(|_| AuthError::WrongCode)
}

fn map_confirmation_err(err: Spake2Error) -> AuthError {
    match err {
        Spake2Error::ConfirmationFailed => AuthError::WrongCode,
        _ => AuthError::Malformed("pake"),
    }
}

/// Viewer 쪽 (SPAKE2 Party A) 진행 상태.
pub struct ViewerPake {
    state: pakery_spake2::PartyAState<Spake2P256>,
}

impl ViewerPake {
    pub fn start(code: &OneTimeCode, host_id: &str) -> Result<(ViewerPake, Vec<u8>), AuthError> {
        let w = code_scalar(code)?;
        let id_b = host_id_bytes(host_id);
        let (pa, state) = PartyA::<Spake2P256>::start(&w, VIEWER_ID, &id_b, AAD, &mut rng())
            .map_err(|_| AuthError::Malformed("pake"))?;
        Ok((ViewerPake { state }, pa))
    }

    pub fn finish(self, pb: &[u8], mac_b: &[u8]) -> Result<(SessionKeys, Vec<u8>), AuthError> {
        let output = self
            .state
            .finish(pb)
            .map_err(|_| AuthError::Malformed("pake point"))?;
        let mac_b = to_mac(mac_b)?;
        output
            .verify_peer_confirmation(&mac_b)
            .map_err(map_confirmation_err)?;
        let keys = derive_session_keys(output.session_key.as_bytes());
        Ok((keys, output.confirmation_mac.to_vec()))
    }
}

/// Host 쪽 (SPAKE2 Party B) 진행 상태.
pub struct HostPake {
    output: pakery_spake2::Spake2Output,
}

impl HostPake {
    pub fn respond(
        code: &OneTimeCode,
        host_id: &str,
        pa: &[u8],
    ) -> Result<(HostPake, Vec<u8>, Vec<u8>), AuthError> {
        let w = code_scalar(code)?;
        let id_b = host_id_bytes(host_id);
        let (pb, state) = PartyB::<Spake2P256>::start(&w, VIEWER_ID, &id_b, AAD, &mut rng())
            .map_err(|_| AuthError::Malformed("pake"))?;
        let output = state
            .finish(pa)
            .map_err(|_| AuthError::Malformed("pake point"))?;
        let mac_b = output.confirmation_mac.to_vec();
        Ok((HostPake { output }, pb, mac_b))
    }

    pub fn confirm(self, mac_a: &[u8]) -> Result<SessionKeys, AuthError> {
        let mac_a = to_mac(mac_a)?;
        self.output
            .verify_peer_confirmation(&mac_a)
            .map_err(map_confirmation_err)?;
        Ok(derive_session_keys(self.output.session_key.as_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(s: &str) -> OneTimeCode {
        OneTimeCode::parse(s).unwrap()
    }

    #[test]
    fn same_code_agrees() {
        let (v, pa) = ViewerPake::start(&code("123456"), "123456789").unwrap();
        let (h, pb, mac_b) = HostPake::respond(&code("123456"), "123456789", &pa).unwrap();
        let (vk, mac_a) = v.finish(&pb, &mac_b).unwrap();
        let hk = h.confirm(&mac_a).unwrap();
        assert_eq!(vk.viewer_to_host, hk.viewer_to_host);
        assert_eq!(vk.hello_binding, hk.hello_binding);
        assert_ne!(vk.viewer_to_host, vk.host_to_viewer);
    }

    #[test]
    fn wrong_code_fails_on_both_sides() {
        let (v, pa) = ViewerPake::start(&code("123456"), "123456789").unwrap();
        let (h, pb, mac_b) = HostPake::respond(&code("654321"), "123456789", &pa).unwrap();
        assert!(matches!(v.finish(&pb, &mac_b), Err(AuthError::WrongCode)));
        assert!(matches!(h.confirm(&[0u8; 32]), Err(AuthError::WrongCode)));
    }

    #[test]
    fn host_id_is_bound() {
        let (v, pa) = ViewerPake::start(&code("123456"), "111111111").unwrap();
        let (_h, pb, mac_b) = HostPake::respond(&code("123456"), "222222222", &pa).unwrap();
        assert!(matches!(v.finish(&pb, &mac_b), Err(AuthError::WrongCode)));
    }

    #[test]
    fn malformed_point_rejected() {
        assert!(matches!(
            HostPake::respond(&code("123456"), "123456789", &[4u8; 65]),
            Err(AuthError::Malformed(_))
        ));
    }
}
