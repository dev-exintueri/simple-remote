// Spike only (throwaway).
use num_bigint::BigUint;
use rand_core_06::{CryptoRng, RngCore};
use spake2::{Ed25519Group, Error, Identity, Password, Spake2};

fn start_pair(pw_a: &[u8], pw_b: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let id_a = Identity::new(b"viewer");
    let id_b = Identity::new(b"host");
    let (sa, msg_a) = Spake2::<Ed25519Group>::start_a(&Password::new(pw_a), &id_a, &id_b);
    let (sb, msg_b) = Spake2::<Ed25519Group>::start_b(&Password::new(pw_b), &id_a, &id_b);
    let ka = sa.finish(&msg_b).unwrap();
    let kb = sb.finish(&msg_a).unwrap();
    (ka, kb)
}

#[test]
fn same_code_gives_equal_32_byte_keys() {
    let (ka, kb) = start_pair(b"123456", b"123456");
    assert_eq!(ka.len(), 32);
    assert_eq!(ka, kb);
}

#[test]
fn different_code_gives_different_keys() {
    let (ka, kb) = start_pair(b"123456", b"123457");
    assert_ne!(ka, kb);
}

#[test]
fn symmetric_mode_roundtrip() {
    let id = Identity::new(b"simple-remote pairing");
    let (s1, m1) = Spake2::<Ed25519Group>::start_symmetric(&Password::new(b"123456"), &id);
    let (s2, m2) = Spake2::<Ed25519Group>::start_symmetric(&Password::new(b"123456"), &id);
    assert_eq!(s1.finish(&m2).unwrap(), s2.finish(&m1).unwrap());
}

#[test]
fn reflected_message_is_rejected() {
    let (s1, m1) = Spake2::<Ed25519Group>::start_a(
        &Password::new(b"123456"),
        &Identity::new(b"viewer"),
        &Identity::new(b"host"),
    );
    assert_eq!(s1.finish(&m1).unwrap_err(), Error::BadSide);
}

/// RNG that yields `scalar_le || 0^32`. Curve25519 `Scalar::random` reads 64 bytes and
/// reduces mod l (from_bytes_mod_order_wide), so this makes the ephemeral scalar exactly
/// `scalar_le` when it is already < l.
struct FixedScalarRng {
    buf: [u8; 64],
}
impl FixedScalarRng {
    fn from_decimal(d: &[u8]) -> Self {
        let le = BigUint::parse_bytes(d, 10).unwrap().to_bytes_le();
        let mut buf = [0u8; 64];
        buf[..le.len()].copy_from_slice(&le);
        Self { buf }
    }
}
impl RngCore for FixedScalarRng {
    fn next_u32(&mut self) -> u32 {
        unimplemented!()
    }
    fn next_u64(&mut self) -> u64 {
        unimplemented!()
    }
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        assert_eq!(dest.len(), 64);
        dest.copy_from_slice(&self.buf);
    }
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core_06::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}
impl CryptoRng for FixedScalarRng {}

/// Same vector as the crate's private `tests::test_asymmetric` (src/lib.rs:700-757), which
/// comes from python-spake2 `test_compat` (NOT RFC 9382). Reproduced here via the public
/// `start_*_with_rng` API.
#[test]
fn python_spake2_compat_vector_via_public_api() {
    let rng_a = FixedScalarRng::from_decimal(
        b"2611694063369306139794446498317402240796898290761098242657700742213257926693",
    );
    let rng_b = FixedScalarRng::from_decimal(
        b"7002393159576182977806091886122272758628412261510164356026361256515836884383",
    );
    let pw = Password::new(b"password");
    let (id_a, id_b) = (Identity::new(b"idA"), Identity::new(b"idB"));

    let (s1, msg1) = Spake2::<Ed25519Group>::start_a_with_rng(&pw, &id_a, &id_b, rng_a);
    assert_eq!(
        hex::encode(&msg1),
        "416fc960df73c9cf8ed7198b0c9534e2e96a5984bfc5edc023fd24dacf371f2af9"
    );
    let (s2, msg2) = Spake2::<Ed25519Group>::start_b_with_rng(&pw, &id_a, &id_b, rng_b);
    assert_eq!(
        hex::encode(&msg2),
        "42354e97b88406922b1df4bea1d7870f17aed3dba7c720b313edae315b00959309"
    );
    let k1 = s1.finish(&msg2).unwrap();
    let k2 = s2.finish(&msg1).unwrap();
    assert_eq!(k1, k2);
    assert_eq!(
        hex::encode(k1),
        "712295de7219c675ddd31942184aa26e0a957cf216bc230d165b215047b520c1"
    );
}
