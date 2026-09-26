// spikes/auth/tests/pakery_rfc9382.rs 의 c_rfc9382_vector1_full 을
// 제품 테스트로 옮긴다 (spec 5.7: 고정 버전의 RFC 9382 동작을 잠근다).
use pakery_core::crypto::{CpaceGroup, Hash, Kdf, Mac};
use pakery_crypto::{HkdfSha256, HmacSha256, P256Group, Sha256Hash, Spake2P256};
use pakery_spake2::encoding::build_transcript;
use pakery_spake2::{PartyA, PartyB};

type A = PartyA<Spake2P256>;
type B = PartyB<Spake2P256>;
type Scalar = <P256Group as CpaceGroup>::Scalar;

fn h(s: &str) -> Vec<u8> {
    hex::decode(s).unwrap()
}

/// 32-byte big-endian RFC scalar -> P-256 scalar through the public API only:
/// scalar_from_wide_bytes reads 64 bytes big-endian and reduces mod n, so
/// 32 zero bytes || s gives s itself (all vector scalars are < n).
fn scalar(s: &str) -> Scalar {
    let mut wide = vec![0u8; 32];
    wide.extend_from_slice(&h(s));
    let sc = P256Group::scalar_from_wide_bytes(&wide).unwrap();
    assert_eq!(hex::encode(P256Group::scalar_to_bytes(&sc)), s);
    sc
}

// RFC 9382 Appendix B, first vector (A="server", B="client"),
// values copied from upstream pakery-tests/tests/spake2_p256_vectors.rs:134-145.
const W: &str = "2ee57912099d31560b3a44b1184b9b4866e904c49d12ac5042c97dca461b1a5f";
const X: &str = "43dd0fd7215bdcb482879fca3220c6a968e66d70b1356cac18bb26c84a78d729";
const Y: &str = "dcb60106f276b02606d8ef0a328c02e4b629f84f89786af5befb0bc75b6e66be";
const PA: &str = "04a56fa807caaa53a4d28dbb9853b9815c61a411118a6fe516a8798434751470f9010153ac33d0d5f2047ffdb1a3e42c9b4e6be662766e1eeb4116988ede5f912c";
const PB: &str = "0406557e482bd03097ad0cbaa5df82115460d951e3451962f1eaf4367a420676d09857ccbc522686c83d1852abfa8ed6e4a1155cf8f1543ceca528afb591a1e0b7";
const K: &str = "0412af7e89717850671913e6b469ace67bd90a4df8ce45c2af19010175e37eed69f75897996d539356e2fa6a406d528501f907e04d97515fbe83db277b715d3325";
const HASH_TT: &str = "0e0672dc86f8e45565d338b0540abe6915bdf72e2b35b5c9e5663168e960a91b";
const KE: &str = "0e0672dc86f8e45565d338b0540abe69";
const KCA: &str = "00c12546835755c86d8c0db7851ae86f";
const KCB: &str = "a9fa3406c3b781b93d804485430ca27a";
const MAC_A: &str = "58ad4aa88e0b60d5061eb6b5dd93e80d9c4f00d127c65b3b35b1b5281fee38f0";
const MAC_B: &str = "d3e2e547f1ae04f2dbdbf0fc4b79f8ecff2dff314b5d32fe9fcef2fb26dc459b";

#[test]
fn c_rfc9382_vector1_full() {
    let (w, x, y) = (scalar(W), scalar(X), scalar(Y));
    let (pa, sa) = A::start_with_scalar(&w, &x, b"server", b"client", b"").unwrap();
    let (pb, sb) = B::start_with_scalar(&w, &y, b"server", b"client", b"").unwrap();
    assert_eq!(hex::encode(&pa), PA);
    assert_eq!(hex::encode(&pb), PB);

    // K is not exposed by PartyXState::finish; recompute K = x*(pB - w*N)
    // with the public group API and check it, then rebuild TT with the
    // crate's own public build_transcript.
    let n = P256Group::from_bytes(<Spake2P256 as pakery_spake2::Spake2Ciphersuite>::N_BYTES).unwrap();
    let k = P256Group::from_bytes(&pb).unwrap().add(&n.scalar_mul(&w).negate()).scalar_mul(&x);
    assert_eq!(hex::encode(k.to_bytes()), K);
    let tt = build_transcript(b"server", b"client", &pa, &pb, &k.to_bytes(), &P256Group::scalar_to_bytes(&w));
    let hash_tt = Sha256Hash::digest(&tt);
    assert_eq!(hex::encode(&hash_tt), HASH_TT);
    let prk = HkdfSha256::extract(&[], &hash_tt[16..]);
    let kc = HkdfSha256::expand(&prk, b"ConfirmationKeys", 32).unwrap();
    assert_eq!(hex::encode(&kc[..16]), KCA);
    assert_eq!(hex::encode(&kc[16..]), KCB);
    assert_eq!(hex::encode(HmacSha256::mac(&kc[..16], &tt).unwrap()), MAC_A);
    assert_eq!(hex::encode(HmacSha256::mac(&kc[16..], &tt).unwrap()), MAC_B);

    // The protocol path itself.
    let oa = sa.finish(&pb).unwrap();
    let ob = sb.finish(&pa).unwrap();
    assert_eq!(hex::encode(oa.session_key.as_bytes()), KE);
    assert_eq!(hex::encode(ob.session_key.as_bytes()), KE);
    assert_eq!(hex::encode(&oa.confirmation_mac), MAC_A);
    assert_eq!(hex::encode(&ob.confirmation_mac), MAC_B);
    oa.verify_peer_confirmation(&ob.confirmation_mac).unwrap();
    ob.verify_peer_confirmation(&oa.confirmation_mac).unwrap();
}
