// Spike only (throwaway).
use snow::{Builder, HandshakeState, Keypair};

const PARAMS: &str = "Noise_KK_25519_ChaChaPoly_BLAKE2s";

fn keypair() -> Keypair {
    Builder::new(PARAMS.parse().unwrap()).generate_keypair().unwrap()
}

fn build(local: &Keypair, remote_pub: &[u8], initiator: bool) -> HandshakeState {
    let b = Builder::new(PARAMS.parse().unwrap())
        .prologue(b"simple-remote v1")
        .unwrap()
        .local_private_key(&local.private)
        .unwrap()
        .remote_public_key(remote_pub)
        .unwrap();
    if initiator {
        b.build_initiator().unwrap()
    } else {
        b.build_responder().unwrap()
    }
}

#[test]
fn kk_handshake_then_transport_both_ways() {
    let (ik, rk) = (keypair(), keypair());
    let mut ini = build(&ik, &rk.public, true);
    let mut res = build(&rk, &ik.public, false);
    let (mut msg, mut out) = (vec![0u8; 65535], vec![0u8; 65535]);

    // -> e, es, ss
    let n = ini.write_message(b"hello", &mut msg).unwrap();
    let p = res.read_message(&msg[..n], &mut out).unwrap();
    assert_eq!(&out[..p], b"hello");
    // <- e, ee, se
    let n = res.write_message(b"", &mut msg).unwrap();
    let p = ini.read_message(&msg[..n], &mut out).unwrap();
    assert_eq!(p, 0);

    assert!(ini.is_handshake_finished() && res.is_handshake_finished());
    assert_eq!(ini.get_handshake_hash(), res.get_handshake_hash());
    assert_eq!(ini.get_handshake_hash().len(), 32); // BLAKE2s HASHLEN
    assert_eq!(ini.get_remote_static().unwrap(), &rk.public[..]);

    let mut ti = ini.into_transport_mode().unwrap();
    let mut tr = res.into_transport_mode().unwrap();

    let n = ti.write_message(b"ping", &mut msg).unwrap();
    assert_eq!(n, 4 + 16); // payload + AEAD tag
    let p = tr.read_message(&msg[..n], &mut out).unwrap();
    assert_eq!(&out[..p], b"ping");

    let n = tr.write_message(b"pong", &mut msg).unwrap();
    let p = ti.read_message(&msg[..n], &mut out).unwrap();
    assert_eq!(&out[..p], b"pong");
}

#[test]
fn wrong_remote_static_fails_handshake() {
    let (ik, rk, other) = (keypair(), keypair(), keypair());
    // Initiator believes the responder's static key is `other`, not `rk`.
    let mut ini = build(&ik, &other.public, true);
    let mut res = build(&rk, &ik.public, false);
    let (mut msg, mut out) = (vec![0u8; 65535], vec![0u8; 65535]);

    let n = ini.write_message(b"hello", &mut msg).unwrap();
    // es/ss mismatch -> payload AEAD fails on the responder
    assert!(matches!(
        res.read_message(&msg[..n], &mut out),
        Err(snow::Error::Decrypt)
    ));
}

#[test]
fn responder_expecting_wrong_initiator_static_fails() {
    let (ik, rk, other) = (keypair(), keypair(), keypair());
    let mut ini = build(&ik, &rk.public, true);
    let mut res = build(&rk, &other.public, false);
    let (mut msg, mut out) = (vec![0u8; 65535], vec![0u8; 65535]);

    let n = ini.write_message(b"", &mut msg).unwrap();
    // Even with an empty payload, the first message carries a 16-byte tag after es+ss.
    assert_eq!(n, 32 + 16);
    assert!(matches!(
        res.read_message(&msg[..n], &mut out),
        Err(snow::Error::Decrypt)
    ));
}
