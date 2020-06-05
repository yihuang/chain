use chain_crypto::CryptoHash;
use chain_crypto_derive::{AsRefCryptoHash, CryptoHasher, PSCCryptoHash};
use parity_scale_codec::Encode;

#[derive(Encode, CryptoHasher, PSCCryptoHash)]
struct Test1 {
    a: String,
}

#[derive(Encode, CryptoHasher, PSCCryptoHash)]
struct Test2 {
    a: String,
}

#[derive(CryptoHasher, AsRefCryptoHash)]
struct PublicKey([u8; 32]);
impl AsRef<[u8]> for PublicKey {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[derive(CryptoHasher, AsRefCryptoHash)]
struct PrivateKey([u8; 32]);
impl AsRef<[u8]> for PrivateKey {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

fn main() {
    let t1 = Test1 {
        a: "test".to_owned(),
    };
    let t2 = Test2 {
        a: "test".to_owned(),
    };
    assert_ne!(t1.hash(), t2.hash());

    let k1 = PublicKey([1_u8; 32]);
    let k2 = PrivateKey([1_u8; 32]);
    assert_ne!(k1.hash(), k2.hash());
}
