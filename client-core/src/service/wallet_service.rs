use std::collections::BTreeSet;

use parity_scale_codec::{Decode, Encode};

use crate::service::WalletState;
use chain_core::common::H256;
use chain_core::init::address::RedeemAddress;
use chain_core::state::account::StakedStateAddress;
use chain_core::tx::data::address::ExtendedAddr;
use client_common::{
    Error, ErrorKind, PublicKey, Result, ResultExt, SecKey, SecureStorage, SecureValueStorage,
    StorageValueType,
};

/// Key space of wallet
const KEYSPACE: &str = "core_wallet";

/// Wallet meta data
#[derive(Debug, Encode, Decode)]
pub struct Wallet {
    /// view key to decrypt enclave transactions
    pub view_key: PublicKey,
    /// public keys to construct transfer addresses
    pub public_keys: BTreeSet<PublicKey>,
    /// public keys of staking addresses
    pub staking_keys: BTreeSet<PublicKey>,
    /// root hashes of multi-sig transfer addresses
    pub root_hashes: BTreeSet<H256>,
}

impl StorageValueType for Wallet {
    #[inline]
    fn keyspace() -> &'static str {
        "core_wallet"
    }
}

impl Wallet {
    /// Creates a new instance of `Wallet`
    pub fn new(view_key: PublicKey) -> Self {
        Self {
            view_key,
            public_keys: Default::default(),
            staking_keys: Default::default(),
            root_hashes: Default::default(),
        }
    }

    /// Returns all staking addresses stored in a wallet
    pub fn staking_addresses(&self) -> BTreeSet<StakedStateAddress> {
        self.staking_keys
            .iter()
            .map(|public_key| StakedStateAddress::BasicRedeem(RedeemAddress::from(public_key)))
            .collect()
    }

    /// Returns all tree addresses stored in a wallet
    pub fn transfer_addresses(&self) -> BTreeSet<ExtendedAddr> {
        self.root_hashes
            .iter()
            .cloned()
            .map(ExtendedAddr::OrTree)
            .collect()
    }

    /// find staking key
    pub fn find_staking_key(&self, redeem_address: &RedeemAddress) -> Option<&PublicKey> {
        self.staking_keys
            .iter()
            .find(|staking_key| &RedeemAddress::from(*staking_key) == redeem_address)
    }

    /// find root hash
    pub fn find_root_hash(&self, address: &ExtendedAddr) -> Option<&H256> {
        match address {
            ExtendedAddr::OrTree(ref root_hash) => {
                self.root_hashes.iter().find(|hash| hash == &root_hash)
            }
        }
    }

    /// Adds a public key to given wallet
    pub fn add_public_key(&mut self, public_key: PublicKey) {
        self.public_keys.insert(public_key);
    }

    /// Adds a public key corresponding to a staking address to given wallet
    pub fn add_staking_key(&mut self, staking_key: PublicKey) {
        self.staking_keys.insert(staking_key);
    }

    /// Adds a multi-sig address to given wallet
    pub fn add_root_hash(&mut self, root_hash: H256) {
        self.root_hashes.insert(root_hash);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secstr::SecUtf8;

    use client_common::storage::MemoryStorage;
    use client_common::{seckey::derive_enckey, PrivateKey, ValueStorage};

    #[test]
    fn check_flow() {
        let storage = MemoryStorage::default();

        let name = "name";
        let enckey = derive_enckey(&SecUtf8::from("passphrase"), name).unwrap();

        let private_key = PrivateKey::new().unwrap();
        let view_key = PublicKey::from(&private_key);

        assert!(storage.get_value_secure(name, &enckey).unwrap().is_none());

        let mut wallet = Wallet::new(view_key.clone());
        storage.create_value_secure(name, &enckey, &wallet).unwrap();

        let error = storage
            .create_value_secure(name, &enckey, view_key.clone())
            .expect_err("Created duplicate value");

        assert_eq!(error.kind(), ErrorKind::InvalidInput);

        assert_eq!(0, wallet.public_keys.len());

        let private_key = PrivateKey::new().unwrap();
        let public_key = PublicKey::from(&private_key);

        wallet.add_public_key(public_key);

        assert_eq!(1, wallet.public_keys.len());

        storage.clear_values::<Wallet>().unwrap();

        assert!(storage.get_value_secure(name, &enckey).unwrap().is_none());
    }
}
