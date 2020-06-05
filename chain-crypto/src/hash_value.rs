#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct HashValue {
    hash: [u8; HashValue::LENGTH],
}

impl HashValue {
    /// The length of the hash in bytes.
    pub const LENGTH: usize = 32;
    /// The length of the hash in bits.
    pub const LENGTH_IN_BITS: usize = Self::LENGTH * 8;
    /// The length of the hash in nibbles.
    pub const LENGTH_IN_NIBBLES: usize = Self::LENGTH * 2;

    /// Create a new [`HashValue`] from a byte array.
    pub fn new(hash: [u8; HashValue::LENGTH]) -> Self {
        HashValue { hash }
    }

    pub fn blake3_of(buffer: &[u8]) -> Self {
        let mut state = blake3::Hasher::new();
        state.update(buffer);
        Self::from_blake3(state)
    }

    fn from_blake3(state: blake3::Hasher) -> Self {
        Self::new(state.finalize().into())
    }
}

impl AsRef<[u8; HashValue::LENGTH]> for HashValue {
    fn as_ref(&self) -> &[u8; HashValue::LENGTH] {
        &self.hash
    }
}

impl From<HashValue> for [u8; HashValue::LENGTH] {
    fn from(hash: HashValue) -> Self {
        hash.hash
    }
}
