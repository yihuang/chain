use bit_vec::BitVec;
use std::collections::BTreeSet;

use parity_scale_codec::Encode;
use secp256k1::schnorrsig::SchnorrSignature;
use secstr::SecUtf8;
use zxcvbn::{feedback::Feedback, zxcvbn as estimate_password_strength};

use chain_core::common::{Proof, H256};
use chain_core::init::address::RedeemAddress;
use chain_core::init::coin::Coin;
use chain_core::state::account::StakedStateAddress;
use chain_core::tx::data::address::ExtendedAddr;
use chain_core::tx::data::attribute::TxAttributes;
use chain_core::tx::data::input::{str2txid, TxoPointer};
use chain_core::tx::data::output::TxOut;
use chain_core::tx::data::{Tx, TxId};
use chain_core::tx::witness::tree::RawPubkey;
use chain_core::tx::witness::{TxInWitness, TxWitness};
use chain_core::tx::{TransactionId, TxAux};
use client_common::tendermint::types::{AbciQueryExt, BlockExt, BroadcastTxResponse};
use client_common::tendermint::{Client, UnauthorizedClient};
use client_common::{
    seckey::derive_enckey, Error, ErrorKind, PrivateKey, PublicKey, Result, ResultExt, SecKey,
    SecureValueStorage, SignedTransaction, Storage, Transaction, TransactionInfo,
};

use crate::service::*;
use crate::transaction_builder::UnauthorizedWalletTransactionBuilder;
use crate::types::{
    AddressType, BalanceChange, TransactionChange, TransactionPending, WalletBalance, WalletKind,
};
use crate::wallet::syncer_logic::create_transaction_change;
use crate::{
    InputSelectionStrategy, Mnemonic, MultiSigWalletClient, UnspentTransactions, WalletClient,
    WalletTransactionBuilder,
};
use client_common::tendermint::types::Time;

/// Default implementation of `WalletClient` based on `Storage` and `Index`
#[derive(Debug, Default, Clone)]
pub struct DefaultWalletClient<S, C, T>
where
    S: Storage,
    C: Client,
    T: WalletTransactionBuilder,
{
    storage: S,
    key_service: KeyService<S>,
    hd_key_service: HdKeyService<S>,
    root_hash_service: RootHashService<S>,
    multi_sig_session_service: MultiSigSessionService<S>,

    tendermint_client: C,
    transaction_builder: T,
}

impl<S, C, T> DefaultWalletClient<S, C, T>
where
    S: Storage,
    C: Client,
    T: WalletTransactionBuilder,
{
    /// Creates a new instance of `DefaultWalletClient`
    pub fn new(storage: S, tendermint_client: C, transaction_builder: T) -> Self {
        Self {
            storage: storage.clone(),
            key_service: KeyService::new(storage.clone()),
            hd_key_service: HdKeyService::new(storage.clone()),
            root_hash_service: RootHashService::new(storage.clone()),
            multi_sig_session_service: MultiSigSessionService::new(storage),
            tendermint_client,
            transaction_builder,
        }
    }
}

impl<S> DefaultWalletClient<S, UnauthorizedClient, UnauthorizedWalletTransactionBuilder>
where
    S: Storage,
{
    /// Creates a new read-only instance of `DefaultWalletClient`
    pub fn new_read_only(storage: S) -> Self {
        Self::new(
            storage,
            UnauthorizedClient,
            UnauthorizedWalletTransactionBuilder,
        )
    }
}

impl<S, C, T> WalletClient for DefaultWalletClient<S, C, T>
where
    S: Storage,
    C: Client,
    T: WalletTransactionBuilder,
{
    #[inline]
    fn wallets(&self) -> Result<Vec<String>> {
        let keys = self.storage.keys(Wallet::keyspace())?;

        keys.into_iter()
            .map(|bytes| {
                String::from_utf8(bytes).chain(|| {
                    (
                        ErrorKind::DeserializationError,
                        "Unable to deserialize wallet names in storage",
                    )
                })
            })
            .collect()
    }

    fn new_wallet(
        &self,
        name: &str,
        passphrase: &SecUtf8,
        wallet_kind: WalletKind,
    ) -> Result<(SecKey, Option<Mnemonic>)> {
        check_passphrase_strength(name, passphrase)?;

        let enckey = derive_enckey(passphrase, name).err_kind(ErrorKind::InvalidInput, || {
            "unable to derive encryption key from passphrase"
        })?;

        match wallet_kind {
            WalletKind::Basic => {
                let private_key = PrivateKey::new()?;
                let view_key = PublicKey::from(&private_key);

                self.key_service
                    .add_keypair(&private_key, &view_key, &enckey)?;

                self.storage
                    .create_value_secure(name, &Wallet::new(view_key), &enckey)
                    .map(|_| None)
            }
            WalletKind::HD => {
                let mnemonic = Mnemonic::new();

                self.hd_key_service.add_mnemonic(name, &mnemonic, &enckey)?;

                let (public_key, private_key) =
                    self.hd_key_service
                        .generate_keypair(name, &enckey, HDAccountType::Viewkey)?;

                self.key_service
                    .add_keypair(&private_key, &public_key, &enckey)?;

                self.storage
                    .create_value_secure(name, &Wallet::new(public_key), &enckey)?;

                Ok((enckey, Some(mnemonic)))
            }
        }
    }

    fn restore_wallet(
        &self,
        name: &str,
        passphrase: &SecUtf8,
        mnemonic: &Mnemonic,
    ) -> Result<SecKey> {
        check_passphrase_strength(name, passphrase)?;

        let enckey = derive_enckey(passphrase, name).err_kind(ErrorKind::InvalidInput, || {
            "unable to derive encryption key from passphrase"
        })?;

        self.hd_key_service.add_mnemonic(name, mnemonic, &enckey)?;

        let (public_key, private_key) =
            self.hd_key_service
                .generate_keypair(name, &enckey, HDAccountType::Viewkey)?;

        self.key_service
            .add_keypair(&private_key, &public_key, &enckey)?;

        self.storage
            .create_value_secure(name, &Wallet::new(public_key), &enckey)
    }

    fn restore_basic_wallet(
        &self,
        name: &str,
        passphrase: &SecUtf8,
        view_key_priv: &PrivateKey,
    ) -> Result<SecKey> {
        check_passphrase_strength(name, passphrase)?;

        let enckey = derive_enckey(passphrase, name).err_kind(ErrorKind::InvalidInput, || {
            "unable to derive encryption key from passphrase"
        })?;

        let view_key = PublicKey::from(view_key_priv);
        self.key_service
            .add_keypair(&view_key_priv, &view_key, passphrase)?;
        self.storage
            .create_value_secure(name, &Wallet::new(view_key), passphrase)
    }

    #[inline]
    fn view_key(&self, name: &str, passphrase: &SecUtf8) -> Result<PublicKey> {
        Ok(self.storage.get_value_secure(name, passphrase)?.view_key)
    }

    #[inline]
    fn view_key_private(&self, name: &str, enckey: &SecKey) -> Result<PrivateKey> {
        self.key_service
            .private_key(
                &self
                    .storage
                    .get_value_secure::<Wallet>(name, passphrase)?
                    .err_kind(ErrorKind::InvalidInput, || "Wallet not found")?
                    .view_key,
                passphrase,
            )?
            .err_kind(ErrorKind::InvalidInput, || "private view key not found")
    }

    #[inline]
    fn public_keys(&self, name: &str, passphrase: &SecUtf8) -> Result<BTreeSet<PublicKey>> {
        Ok(self
            .storage
            .get_value_secure::<Wallet>(name, passphrase)?
            .err_kind(ErrorKind::InvalidInput, || "Wallet not found")?
            .public_keys)
    }

    #[inline]
    fn staking_keys(&self, name: &str, passphrase: &SecUtf8) -> Result<BTreeSet<PublicKey>> {
        Ok(self
            .storage
            .get_value_secure::<Wallet>(name, passphrase)?
            .err_kind(ErrorKind::InvalidInput, || "Wallet not found")?
            .staking_keys)
    }

    #[inline]
    fn root_hashes(&self, name: &str, passphrase: &SecUtf8) -> Result<BTreeSet<H256>> {
        Ok(self
            .storage
            .get_value_secure::<Wallet>(name, passphrase)?
            .err_kind(ErrorKind::InvalidInput, || "Wallet not found")?
            .root_hashes)
    }

    #[inline]
    fn staking_addresses(
        &self,
        name: &str,
        enckey: &SecKey,
    ) -> Result<BTreeSet<StakedStateAddress>> {
        Ok(self
            .storage
            .get_value_secure::<Wallet>(name, passphrase)?
            .err_kind(ErrorKind::InvalidInput, || "Wallet not found")?
            .staking_addresses())
    }

    #[inline]
    fn transfer_addresses(
        &self,
        name: &str,
        passphrase: &SecUtf8,
    ) -> Result<BTreeSet<ExtendedAddr>> {
        Ok(self
            .storage
            .get_value_secure::<Wallet>(name, passphrase)?
            .err_kind(ErrorKind::InvalidInput, || "Wallet not found")?
            .transfer_addresses())
    }

    #[inline]
    fn find_staking_key(
        &self,
        name: &str,
        enckey: &SecKey,
        redeem_address: &RedeemAddress,
    ) -> Result<Option<PublicKey>> {
        Ok(self
            .storage
            .get_value_secure::<Wallet>(name, passphrase)?
            .err_kind(ErrorKind::InvalidInput, || "Wallet not found")?
            .find_staking_key(redeem_address)
            .copied())
    }

    #[inline]
    fn find_root_hash(
        &self,
        name: &str,
        enckey: &SecKey,
        address: &ExtendedAddr,
    ) -> Result<Option<H256>> {
        Ok(self
            .storage
            .get_value_secure::<Wallet>(name, passphrase)?
            .err_kind(ErrorKind::InvalidInput, || "Wallet not found")?
            .find_root_hash(address)
            .copied())
    }

    #[inline]
    fn private_key(&self, enckey: &SecKey, public_key: &PublicKey) -> Result<Option<PrivateKey>> {
        self.key_service.private_key(public_key, enckey)
    }

    fn new_public_key(
        &self,
        name: &str,
        enckey: &SecKey,
        address_type: Option<AddressType>,
    ) -> Result<PublicKey> {
        let (public_key, private_key) = if self.hd_key_service.has_wallet(name)? {
            self.hd_key_service.generate_keypair(
                name,
                enckey,
                address_type
                    .chain(|| {
                        (
                            ErrorKind::InvalidInput,
                            "Address type is needed when creating address for HD wallet",
                        )
                    })?
                    .into(),
            )?
        } else {
            let private_key = PrivateKey::new()?;
            let public_key = PublicKey::from(&private_key);

            (public_key, private_key)
        };

        self.key_service
            .add_keypair(&private_key, &public_key, enckey)?;

        self.storage
            .modify_value_secure_strict(name, passphrase, |wallet: &mut Wallet| {
                wallet.add_public_key(public_key)
            })?;

        Ok(public_key)
    }

    fn new_staking_address(&self, name: &str, enckey: &SecKey) -> Result<StakedStateAddress> {
        let (staking_key, private_key) = if self.hd_key_service.has_wallet(name)? {
            self.hd_key_service
                .generate_keypair(name, enckey, HDAccountType::Staking)?
        } else {
            let private_key = PrivateKey::new()?;
            let public_key = PublicKey::from(&private_key);

            (public_key, private_key)
        };

        self.key_service
            .add_keypair(&private_key, &staking_key, enckey)?;

        self.storage
            .modify_value_secure_strict(name, passphrase, |wallet: &mut Wallet| {
                wallet.add_staking_key(staking_key)
            })?;

        Ok(StakedStateAddress::BasicRedeem(RedeemAddress::from(
            &staking_key,
        )))
    }

    fn new_transfer_address(&self, name: &str, enckey: &SecKey) -> Result<ExtendedAddr> {
        let (public_key, private_key) = if self.hd_key_service.has_wallet(name)? {
            self.hd_key_service
                .generate_keypair(name, enckey, HDAccountType::Transfer)?
        } else {
            let private_key = PrivateKey::new()?;
            let public_key = PublicKey::from(&private_key);

            (public_key, private_key)
        };

        self.key_service
            .add_keypair(&private_key, &public_key, enckey)?;

        self.storage
            .modify_value_secure_strict(name, passphrase, |wallet: &mut Wallet| {
                Ok(wallet.add_public_key(public_key.clone()))
            })?;

        self.new_multisig_transfer_address(name, enckey, vec![public_key.clone()], public_key, 1)
    }

    fn new_watch_staking_address(
        &self,
        name: &str,
        enckey: &SecKey,
        public_key: &PublicKey,
    ) -> Result<StakedStateAddress> {
        self.storage
            .modify_value_secure_strict(name, passphrase, |wallet: &mut Wallet| {
                Ok(wallet.add_staking_key(public_key))
            })?;

        Ok(StakedStateAddress::BasicRedeem(RedeemAddress::from(
            public_key,
        )))
    }

    fn new_watch_transfer_address(
        &self,
        name: &str,
        enckey: &SecKey,
        public_key: &PublicKey,
    ) -> Result<ExtendedAddr> {
        self.new_multisig_transfer_address(
            name,
            enckey,
            vec![public_key.clone()],
            public_key.clone(),
            1,
        )
    }

    fn new_multisig_transfer_address(
        &self,
        name: &str,
        enckey: &SecKey,
        public_keys: Vec<PublicKey>,
        self_public_key: PublicKey,
        m: usize,
    ) -> Result<ExtendedAddr> {
        if !public_keys.contains(&self_public_key) {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Signer public keys does not contain self public key",
            ));
        }

        let (root_hash, multi_sig_address) =
            self.root_hash_service
                .new_root_hash(public_keys, self_public_key, m, enckey)?;

        self.wallet_service.add_root_hash(name, enckey, root_hash)?;

        Ok(multi_sig_address.into())
    }

    fn generate_proof(
        &self,
        name: &str,
        enckey: &SecKey,
        address: &ExtendedAddr,
        public_keys: Vec<PublicKey>,
    ) -> Result<Proof<RawPubkey>> {
        // To verify if the enckey is correct or not
        self.wallet_service.view_key(name, enckey)?;

        match address {
            ExtendedAddr::OrTree(ref address) => {
                self.root_hash_service
                    .generate_proof(address, public_keys, enckey)
            }
        }
    }

    fn required_cosigners(
        &self,
        name: &str,
        passphrase: &SecUtf8,
        root_hash: &H256,
    ) -> Result<usize> {
        // To verify if the passphrase is correct or not
        self.storage.get_value_secure::<Wallet>(name, passphrase)?;

        self.root_hash_service.required_signers(root_hash, enckey)
    }

    #[inline]
    fn balance(&self, name: &str, enckey: &SecKey) -> Result<WalletBalance> {
        // Check if wallet exists
        self.storage.get_value_secure::<Wallet>(name, passphrase)?;
        Ok(self
            .storage
            .get_value_secure(name, passphrase)?
            .map(|state: WalletState| state.get_balance())
            .transpose()?
            .unwrap_or_default())
    }

    fn history(
        &self,
        name: &str,
        enckey: &SecKey,
        offset: usize,
        limit: usize,
        reversed: bool,
    ) -> Result<Vec<TransactionChange>> {
        // Check if wallet exists
        self.wallet_service.view_key(name, enckey)?;

        Ok(self
            .storage
            .get_value_secure::<WalletState>(name, passphrase)?
            .unwrap_or_default()
            .get_transaction_history(reversed)
            .filter(|change| BalanceChange::NoChange != change.balance_change)
            .skip(offset)
            .take(limit)
            .collect::<Vec<_>>())
    }

    fn unspent_transactions(&self, name: &str, enckey: &SecKey) -> Result<UnspentTransactions> {
        // Check if wallet exists
        self.wallet_service.view_key(name, enckey)?;

        let unspent_transactions = self
            .storage
            .get_value_secure::<WalletState>(name, passphrase)?
            .map(|state| state.get_unspent_transactions(false).into_iter().collect())
            .unwrap_or_default();

        Ok(UnspentTransactions::new(unspent_transactions))
    }

    fn has_unspent_transactions(
        &self,
        name: &str,
        enckey: &SecKey,
        inputs: &[TxoPointer],
    ) -> Result<bool> {
        // Check if wallet exists
        self.wallet_service.view_key(name, enckey)?;

        Ok(self
            .storage
            .get_value_secure::<WalletState>(name, passphrase)?
            .map(|state| state.has_unspent_transactions(inputs))
            .unwrap_or(false))
    }

    #[inline]
    fn output(&self, name: &str, enckey: &SecKey, input: &TxoPointer) -> Result<TxOut> {
        // Check if wallet exists
        self.wallet_service.view_key(name, enckey)?;

        self.storage
            .get_value_secure::<WalletState>(name, passphrase)?
            .map(|state| state.get_output(input))
            .transpose()?
            .flatten()
            .err_kind(ErrorKind::InvalidInput, || {
                "Output details not found for given transaction input"
            })
    }

    fn create_transaction(
        &self,
        name: &str,
        enckey: &SecKey,
        outputs: Vec<TxOut>,
        attributes: TxAttributes,
        input_selection_strategy: Option<InputSelectionStrategy>,
        return_address: ExtendedAddr,
    ) -> Result<(TxAux, Vec<TxoPointer>, Coin)> {
        let mut unspent_transactions = self.unspent_transactions(name, enckey)?;
        unspent_transactions.apply_all(input_selection_strategy.unwrap_or_default().as_ref());

        self.transaction_builder.build_transfer_tx(
            name,
            enckey,
            unspent_transactions,
            outputs,
            return_address,
            attributes,
        )
    }

    #[inline]
    fn broadcast_transaction(&self, tx_aux: &TxAux) -> Result<BroadcastTxResponse> {
        self.tendermint_client
            .broadcast_transaction(&tx_aux.encode())
    }

    fn export_plain_tx(&self, name: &str, enckey: &SecKey, txid: &str) -> Result<String> {
        let txid = str2txid(txid).chain(|| (ErrorKind::InvalidInput, "invalid transaction id"))?;
        let public_key = self.view_key(name, enckey)?;
        let private_key = self
            .private_key(enckey, &public_key)?
            .chain(|| (ErrorKind::StorageError, "can not find private key"))?;
        let tx = self.transaction_builder.decrypt_tx(txid, &private_key)?;
        // get the block height
        let tx_change = self
            .storage
            .get_value_secure::<WalletState>(name, passphrase)?
            .map(|state| {
                state
                    .get_transaction_history(false)
                    .filter(|change| BalanceChange::NoChange != change.balance_change)
                    .find(|tx_change| tx_change.transaction_id == tx.id())
            })
            .flatten()
            .err_kind(ErrorKind::InvalidInput, || {
                "no transaction find by transaction id"
            })?;

        let tx_info = TransactionInfo {
            tx,
            block_height: tx_change.block_height,
        };

        let tx_str = serde_json::to_string(&tx_info)
            .chain(|| (ErrorKind::InvalidInput, "invalid transaction id"))?;
        Ok(base64::encode(&tx_str))
    }

    /// import a plain base64 encoded plain transaction
    fn import_plain_tx(&self, name: &str, enckey: &SecKey, tx_str: &str) -> Result<Coin> {
        let tx_raw = base64::decode(tx_str)
            .chain(|| (ErrorKind::DecryptionError, "Unable to decrypt transaction"))?;
        let tx_info: TransactionInfo = serde_json::from_slice(&tx_raw)
            .chain(|| (ErrorKind::DecryptionError, "Unable to decrypt transaction"))?;
        // check if the output is spent or not
        let v = self
            .tendermint_client
            .query("meta", &tx_info.tx.id().to_vec())?
            .bytes()?;
        let bit_flag = BitVec::from_bytes(&v);
        let spent_flags: Result<Vec<bool>> = tx_info
            .tx
            .outputs()
            .iter()
            .enumerate()
            .map(|(index, _output)| {
                bit_flag
                    .get(index)
                    .chain(|| (ErrorKind::InvalidInput, "check failed in enclave"))
            })
            .collect();
        let mut memento = WalletStateMemento::default();
        // check if tx belongs to the block
        let block = self.tendermint_client.block(tx_info.block_height)?;
        if !block.enclave_transaction_ids()?.contains(&tx_info.tx.id()) {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "block height and transaction not match",
            ));
        }
        let wallet = self.wallet_service.get_wallet(name, enckey)?;

        let wallet_state = self.wallet_service.get_wallet_state(name, enckey)?;

        let imported_value = import_transaction(
            &wallet,
            &wallet_state,
            &mut memento,
            &tx_info.tx,
            tx_info.block_height,
            block.header.time,
            spent_flags?,
        )
        .chain(|| (ErrorKind::InvalidInput, "import error"))?;

        self.storage
            .modify_value_secure(name, passphrase, |state: &mut WalletState| {
                state.apply_memento(&memento)
            })?;
        Ok(imported_value)
    }

    fn get_current_block_height(&self) -> Result<u64> {
        let status = self.tendermint_client.status()?;
        let current_block_height = status.sync_info.latest_block_height.value();
        Ok(current_block_height)
    }

    fn update_tx_pending_state(
        &self,
        name: &str,
        enckey: &SecKey,
        tx_id: TxId,
        tx_pending: TransactionPending,
    ) -> Result<()> {
        let mut wallet_state_memento = WalletStateMemento::default();
        wallet_state_memento.add_pending_transaction(tx_id, tx_pending);
        self.storage
            .modify_value_secure(name, passphrase, |state: &mut WalletState| {
                state.apply_memento(&wallet_state_memento)
            })
    }
}

impl<S, C, T> MultiSigWalletClient for DefaultWalletClient<S, C, T>
where
    S: Storage,
    C: Client,
    T: WalletTransactionBuilder,
{
    fn schnorr_signature(
        &self,
        name: &str,
        enckey: &SecKey,
        message: &H256,
        public_key: &PublicKey,
    ) -> Result<SchnorrSignature> {
        // To verify if the enckey is correct or not
        self.transfer_addresses(name, enckey)?;

        let private_key = self.private_key(enckey, public_key)?.chain(|| {
            (
                ErrorKind::InvalidInput,
                format!("Public key ({}) is not owned by current wallet", public_key),
            )
        })?;
        private_key.schnorr_sign(message)
    }

    fn new_multi_sig_session(
        &self,
        name: &str,
        enckey: &SecKey,
        message: H256,
        signer_public_keys: Vec<PublicKey>,
        self_public_key: PublicKey,
    ) -> Result<H256> {
        // To verify if the enckey is correct or not
        self.transfer_addresses(name, enckey)?;

        let self_private_key = self.private_key(enckey, &self_public_key)?.chain(|| {
            (
                ErrorKind::InvalidInput,
                format!(
                    "Self public key ({}) is not owned by current wallet",
                    self_public_key
                ),
            )
        })?;

        self.multi_sig_session_service.new_session(
            message,
            signer_public_keys,
            self_public_key,
            self_private_key,
            enckey,
        )
    }

    fn nonce_commitment(&self, session_id: &H256, enckey: &SecKey) -> Result<H256> {
        self.multi_sig_session_service
            .nonce_commitment(session_id, enckey)
    }

    fn add_nonce_commitment(
        &self,
        session_id: &H256,
        enckey: &SecKey,
        nonce_commitment: H256,
        public_key: &PublicKey,
    ) -> Result<()> {
        self.multi_sig_session_service.add_nonce_commitment(
            session_id,
            nonce_commitment,
            public_key,
            enckey,
        )
    }

    fn nonce(&self, session_id: &H256, enckey: &SecKey) -> Result<PublicKey> {
        self.multi_sig_session_service.nonce(session_id, enckey)
    }

    fn add_nonce(
        &self,
        session_id: &H256,
        enckey: &SecKey,
        nonce: &PublicKey,
        public_key: &PublicKey,
    ) -> Result<()> {
        self.multi_sig_session_service
            .add_nonce(session_id, &nonce, public_key, enckey)
    }

    fn partial_signature(&self, session_id: &H256, enckey: &SecKey) -> Result<H256> {
        self.multi_sig_session_service
            .partial_signature(session_id, enckey)
    }

    fn add_partial_signature(
        &self,
        session_id: &H256,
        enckey: &SecKey,
        partial_signature: H256,
        public_key: &PublicKey,
    ) -> Result<()> {
        self.multi_sig_session_service.add_partial_signature(
            session_id,
            partial_signature,
            public_key,
            enckey,
        )
    }

    fn signature(&self, session_id: &H256, enckey: &SecKey) -> Result<SchnorrSignature> {
        self.multi_sig_session_service.signature(session_id, enckey)
    }

    fn transaction(
        &self,
        name: &str,
        session_id: &H256,
        enckey: &SecKey,
        unsigned_transaction: Tx,
    ) -> Result<TxAux> {
        if unsigned_transaction.inputs.len() != 1 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Multi-Sig Signing is only supported for transactions with only one input",
            ));
        }

        let output_to_spend = self.output(name, enckey, &unsigned_transaction.inputs[0])?;
        let root_hash = self
            .wallet_service
            .find_root_hash(name, enckey, &output_to_spend.address)?
            .chain(|| {
                (
                    ErrorKind::IllegalInput,
                    "Output address is not owned by current wallet; cannot spend output in given transaction",
                )
            })?;
        let public_keys = self
            .multi_sig_session_service
            .public_keys(session_id, enckey)?;

        let proof = self
            .root_hash_service
            .generate_proof(&root_hash, public_keys, enckey)?;
        let signature = self.signature(session_id, enckey)?;

        let witness = TxWitness::from(vec![TxInWitness::TreeSig(signature, proof)]);
        let signed_transaction =
            SignedTransaction::TransferTransaction(unsigned_transaction, witness);

        self.transaction_builder.obfuscate(signed_transaction)
    }
}

fn check_passphrase_strength(name: &str, passphrase: &SecUtf8) -> Result<()> {
    // `estimate_password_strength` returns a score between `0-4`. Any score less than 3 should be considered too
    // weak.
    let password_entropy = estimate_password_strength(passphrase.unsecure(), &[name])
        .chain(|| (ErrorKind::IllegalInput, "Blank passphrase"))?;

    #[cfg(debug_assertions)]
    let entropy_score = 0;
    #[cfg(not(debug_assertions))]
    let entropy_score = 3;

    if password_entropy.score() < entropy_score {
        return Err(Error::new(
            ErrorKind::IllegalInput,
            format!(
                "Weak passphrase: {}",
                parse_feedback(password_entropy.feedback().as_ref())
            ),
        ));
    }

    Ok(())
}

fn parse_feedback(feedback: Option<&Feedback>) -> String {
    match feedback {
        None => "No feedback available!".to_string(),
        Some(feedback) => {
            let mut feedbacks = Vec::new();

            if let Some(warning) = feedback.warning() {
                feedbacks.push(format!("Warning: {}", warning));
            }

            for suggestion in feedback.suggestions() {
                feedbacks.push(format!("Suggestion: {}", suggestion));
            }

            if feedbacks.is_empty() {
                feedbacks.push("No feedback available!".to_string());
            }

            feedbacks.join(" | ")
        }
    }
}

fn import_transaction(
    wallet: &Wallet,
    wallet_state: &WalletState,
    memento: &mut WalletStateMemento,
    transaction: &Transaction,
    block_height: u64,
    block_time: Time,
    spent_flag: Vec<bool>,
) -> Result<Coin> {
    let transaction_change =
        create_transaction_change(wallet, wallet_state, transaction, block_height, block_time)
            .chain(|| (ErrorKind::InvalidInput, "create transaction change failed"))?;
    let mut value = Coin::zero();
    let transfer_addresses = wallet.transfer_addresses();
    for (i, (output, spent)) in transaction_change
        .outputs
        .iter()
        .zip(spent_flag)
        .enumerate()
    {
        // Only add unspent transaction if output address belongs to current wallet
        if transfer_addresses.contains(&output.address) && !spent {
            memento.add_unspent_transaction(
                TxoPointer::new(transaction_change.transaction_id, i),
                output.clone(),
            );
            value = (value + output.value).chain(|| {
                (
                    ErrorKind::InvalidInput,
                    "invalid coin in outputs of transaction",
                )
            })?;
        }
    }
    memento.add_transaction_change(transaction_change);
    Ok(value)
}
