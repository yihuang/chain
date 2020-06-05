use crate::HashValue;

const CHAIN_HASH_PREFIX: &[u8] = b"CRYPTOCHAIN:";

#[derive(Clone)]
pub struct DefaultHasher {
    state: blake3::Hasher,
}

impl DefaultHasher {
    #[doc(hidden)]
    /// This function does not return a HashValue in the sense of our usual
    /// hashes, but a construction of initial bytes that are fed into any hash
    /// provided we're passed  a (lcs) serialization name as argument.
    pub fn prefixed_hash(buffer: &[u8]) -> [u8; HashValue::LENGTH] {
        // The salt is initial material we prefix to actual value bytes for
        // domain separation. Its length is variable.
        let salt: Vec<u8> = [CHAIN_HASH_PREFIX, buffer].concat();
        // The seed is a fixed-length hash of the salt, thereby preventing
        // suffix attacks on the domain separation bytes.
        HashValue::blake3_of(&salt[..]).into()
    }

    #[doc(hidden)]
    pub fn new(typename: &[u8]) -> Self {
        let mut state = blake3::Hasher::new();
        if !typename.is_empty() {
            state.update(&Self::prefixed_hash(typename));
        }
        DefaultHasher { state }
    }

    #[doc(hidden)]
    pub fn update(&mut self, bytes: &[u8]) {
        self.state.update(bytes);
    }

    #[doc(hidden)]
    pub fn finish(self) -> HashValue {
        HashValue::new(self.state.finalize().into())
    }
}

/// A trait for representing the state of a cryptographic hasher.
pub trait CryptoHasher: Default {
    /// Write bytes into the hasher.
    fn update(&mut self, bytes: &[u8]);

    /// Finish constructing the [`HashValue`].
    fn finish(self) -> HashValue;
}

/// A type that can be cryptographically hashed to produce a `HashValue`.
///
/// In most cases, this trait should not be implemented manually but rather derived using
/// the macros `serde::Serialize`, `CryptoHasher`, and `LCSCryptoHash`.
pub trait HasCryptoHasher {
    /// The associated `Hasher` type which comes with a unique salt for this type.
    type Hasher: CryptoHasher;
}

pub trait CryptoHash {
    /// Hashes the object and produces a `HashValue`.
    fn hash(&self) -> HashValue;
}
