use parity_scale_codec::{Decode, Encode};
use std::collections::BTreeMap;

use chain_core::{
    init::coin::{sum_coins, CoinError},
    tx::data::{input::TxoPointer, output::TxOut, TxId},
};
use client_common::{
    Error, ErrorKind, Result, ResultExt, SecureValueStorage, Storage, StorageValueType,
    ValueStorage, SecKey
};

use crate::types::{TransactionChange, TransactionPending, WalletBalance};

<<<<<<< Updated upstream
/// Maintains mapping `wallet-name -> wallet-state`
#[derive(Debug, Default, Clone)]
pub struct WalletStateService<S>
where
    S: Storage,
{
    storage: S,
}

impl<S> WalletStateService<S>
where
    S: Storage,
{
    /// Creates new instance of global state service
    #[inline]
    pub fn new(storage: S) -> Self {
        Self { storage }
    }

    /// Clears all storage
    #[inline]
    pub fn clear(&self) -> Result<()> {
        self.storage.clear_values::<WalletState>()
    }

    /// Returns `true` if given transaction inputs are present in the list of unspent transactions, `false` otherwise
    pub fn has_unspent_transactions(
        &self,
        name: &str,
        enckey: &SecKey,
        inputs: &[TxoPointer],
    ) -> Result<bool> {
        let unspent_transactions = self.get_unspent_transactions(name, enckey, false)?;

        Ok(inputs
            .iter()
            .all(|input| unspent_transactions.contains_key(input)))
    }

    /// Returns currently stored unspent transactions for given wallet
    /// if include_pending is true, get all the unspent transactions, else just available transactions
    #[inline]
    pub fn get_unspent_transactions(
        &self,
        name: &str,
        enckey: &SecKey,
        include_pending: bool,
    ) -> Result<BTreeMap<TxoPointer, TxOut>> {
        let wallet_state = self.get_wallet_state(name, enckey)?;
        if include_pending {
            Ok(wallet_state.unspent_transactions)
        } else {
            Ok(wallet_state.get_available_transactions())
        }
    }

    /// Returns currently stored transaction history for given wallet
    #[inline]
    pub fn get_transaction_history(
        &self,
        name: &str,
        enckey: &SecKey,
        reversed: bool,
    ) -> Result<Box<dyn Iterator<Item = TransactionChange>>> {
        let mut state = self.get_wallet_state(name, enckey)?;
        let mut history = std::mem::replace(&mut state.transaction_history, BTreeMap::new());
        let get_tx = move |txid| history.remove(&txid);
        let iter = state.transaction_log.into_iter();
        Ok(if reversed {
            Box::new(iter.rev().filter_map(get_tx))
        } else {
            Box::new(iter.filter_map(get_tx))
        })
    }

    /// Returns currently stored transaction change for given wallet and transaction id
    #[inline]
    pub fn get_transaction_change(
        &self,
        name: &str,
        enckey: &SecKey,
        transaction_id: &TxId,
    ) -> Result<Option<TransactionChange>> {
        Ok(self
            .get_wallet_state(name, enckey)?
            .get_transaction_change(transaction_id))
    }

    /// Returns details corresponding to given input
    pub fn get_output(
        &self,
        name: &str,
        enckey: &SecKey,
        input: &TxoPointer,
    ) -> Result<Option<TxOut>> {
        self.get_wallet_state(name, enckey)?.get_output(input)
    }

    /// Returns currently stored balance for given wallet
    pub fn get_balance(&self, name: &str, enckey: &SecKey) -> Result<WalletBalance> {
        let wallet_state = self.get_wallet_state(name, enckey)?;
        let balance = wallet_state
            .get_balance()
            .chain(|| (ErrorKind::StorageError, "Calculate balance error"))?;
        Ok(balance)
    }
}

fn parse_wallet_state<T: AsRef<[u8]>>(
    name: &str,
    bytes_optional: Option<T>,
) -> Result<WalletState> {
    bytes_optional
        .map(|bytes| {
            WalletState::decode(&mut bytes.as_ref()).chain(|| {
                (
                    ErrorKind::DeserializationError,
                    format!(
                        "Unable to deserialize wallet state for wallet with name {}",
                        name
                    ),
                )
            })
        })
        .transpose()
        .map(|wallet_state_optional| wallet_state_optional.unwrap_or_default())
}

/// Load wallet state from storage
pub fn load_wallet_state<S: SecureStorage>(
    storage: &S,
    name: &str,
    enckey: &SecKey,
) -> Result<Option<WalletState>> {
    storage.load_secure(KEYSPACE, name, enckey)
}

/// Save wallet state to storage
pub fn save_wallet_state<S: SecureStorage>(
    storage: &S,
    name: &str,
    enckey: &SecKey,
    state: &WalletState,
) -> Result<()> {
    storage.save_secure(KEYSPACE, name, enckey, state)
}

/// Modify wallet state atomically, and returns the new one.
pub fn modify_wallet_state<S, F>(
    storage: &S,
    name: &str,
    enckey: &SecKey,
    f: F,
) -> Result<WalletState>
where
    S: SecureStorage,
    F: Fn(&mut WalletState) -> Result<()>,
{
    storage.fetch_and_update_secure(KEYSPACE, name, enckey, |bytes_optional| {
        let mut wallet_state = parse_wallet_state(name, bytes_optional)?;
        f(&mut wallet_state)?;
        Ok(Some(wallet_state.encode()))
    })?;
    // FIXME need to modify the storage trait to save this extra loading.
    Ok(load_wallet_state(storage, name, enckey)?.unwrap())
}

/// Delete wallet state from storage
pub fn delete_wallet_state<S: Storage>(storage: &S, name: &str) -> Result<()> {
    storage.delete(KEYSPACE, name)?;
    Ok(())
}

=======
    pub fn delete_wallet_state(&self, name: &str, passphrase: &SecUtf8) -> Result<()> {
        // Check if the passphrase is correct
        let _ = self.get_wallet_state(name, passphrase)?;
        self.storage.delete_value::<WalletState>(name)
    }

    #[inline]
    fn get_wallet_state(&self, name: &str, passphrase: &SecUtf8) -> Result<WalletState> {
        Ok(self
            .storage
            .get_value_secure(name, passphrase)?
            .unwrap_or_default())
    }
}

>>>>>>> value storage
=======
>>>>>>> Stashed changes
/// Wallet state
#[derive(Debug, Encode, Decode)]
pub struct WalletState {
    /// UTxO
    pub unspent_transactions: BTreeMap<TxoPointer, TxOut>,
    /// Transaction pending information indexed by txid
    pub pending_transactions: BTreeMap<TxId, TransactionPending>,
    /// Transaction history indexed by txid
    pub transaction_history: BTreeMap<TxId, TransactionChange>,
    /// Transaction ids ordered by insert order.
    pub transaction_log: Vec<TxId>,
}

impl Default for WalletState {
    #[inline]
    fn default() -> WalletState {
        WalletState {
            unspent_transactions: Default::default(),
            pending_transactions: Default::default(),
            transaction_history: Default::default(),
            transaction_log: vec![],
        }
    }
}

impl StorageValueType for WalletState {
    #[inline]
    fn keyspace() -> &'static str {
        "core_wallet_state"
    }
}

impl WalletState {
    /// if the txid can not be found in the latest `block_height_ensure` blocks after it broadcast
    /// we need to rollback
    pub fn get_rollback_pending_tx(
        &self,
        current_block_height: u64,
        block_height_ensure: u64,
    ) -> Vec<TxId> {
        self.pending_transactions
            .iter()
            .filter_map(|(key, value)| {
                if value.block_height + block_height_ensure < current_block_height {
                    Some(*key)
                } else {
                    None
                }
            })
            .collect()
    }

    fn get_pending_inputs(&self) -> Vec<TxoPointer> {
        self.pending_transactions
            .values()
            .map(|value| value.used_inputs.clone())
            .flatten()
            .collect()
    }
    /// get transactions which in unspent_transactions and not in pending_transactions
    pub fn get_available_transactions(&self) -> BTreeMap<TxoPointer, TxOut> {
        let pending_inputs = self.get_pending_inputs();
        let mut result = BTreeMap::new();
        let _ = self
            .unspent_transactions
            .iter()
            .filter(|(key, _value)| !pending_inputs.contains(key))
            .map(|(key, value)| result.insert(key.clone(), value.clone()))
            .collect::<Vec<_>>();
        result
    }
    /// get the balance info
    pub fn get_balance(&self) -> std::result::Result<WalletBalance, CoinError> {
        // pending amount
        let pending_coins = self
            .pending_transactions
            .values()
            .map(|value| value.return_amount);
        let amount_pending = sum_coins(pending_coins)?;

        // unavailable amount
        let pending_inputs = self.get_pending_inputs();
        let available_coins = self
            .unspent_transactions
            .iter()
            .filter(|(key, _value)| !pending_inputs.contains(key))
            .map(|(_key, value)| value.value);
        let amount_available = sum_coins(available_coins)?;

        // total amount
        let amount_total = (amount_pending + amount_available)?;

        let wallet_balances = WalletBalance {
            total: amount_total,
            available: amount_available,
            pending: amount_pending,
        };
        Ok(wallet_balances)
    }
    /// Applies memento to wallet state
    pub fn apply_memento(&mut self, memento: &WalletStateMemento) -> Result<()> {
        for operation in memento.0.iter() {
            self.apply_memento_operation(operation)?;
        }
        Ok(())
    }

    fn add_transaction_change(&mut self, txid: TxId, change: TransactionChange) {
        self.transaction_history.insert(txid, change);
        self.transaction_log.push(txid);
    }

    /// Applies a memento operation to wallet state
    fn apply_memento_operation(&mut self, memento_operation: &MementoOperation) -> Result<()> {
        match memento_operation {
            MementoOperation::AddTransactionChange(ref transaction_id, ref transaction_change) => {
                if !self.transaction_history.contains_key(transaction_id) {
                    self.add_transaction_change(transaction_id.clone(), transaction_change.clone());
                }
            }
            MementoOperation::AddUnspentTransaction(ref input, ref output) => {
                self.unspent_transactions
                    .insert(input.clone(), output.clone());
            }
            MementoOperation::RemoveUnspentTransaction(ref input) => {
                self.unspent_transactions.remove(input);
            }
            MementoOperation::AddPendingTransaction(ref transaction_id, ref pending_info) => {
                if !self.pending_transactions.contains_key(transaction_id) {
                    let _ = self
                        .pending_transactions
                        .insert(*transaction_id, pending_info.clone());
                }
            }
            MementoOperation::RemovePendingTransaction(ref transaction_id) => {
                self.pending_transactions.remove(transaction_id);
            }
        }
        Ok(())
    }

    /// Returns currently stored transaction change for given wallet and transaction id
    pub fn get_transaction_change(&self, transaction_id: &TxId) -> Option<TransactionChange> {
        self.transaction_history.get(transaction_id).cloned()
    }

    /// Returns details corresponding to given input
    pub fn get_output(&self, input: &TxoPointer) -> Result<Option<TxOut>> {
        if let Some(change) = self.get_transaction_change(&input.id) {
            if change.outputs.len() > input.index as usize {
                Ok(Some(change.outputs[input.index as usize].clone()))
            } else {
                Err(Error::new(
                    ErrorKind::InvalidInput,
                    "Index is greater than total outputs in transaction",
                ))
            }
        } else {
            Ok(None)
        }
    }

    /// Returns currently stored transaction history for given wallet
    #[inline]
    pub fn get_transaction_history(
        &self,
        reversed: bool,
    ) -> Box<dyn Iterator<Item = TransactionChange>> {
        let mut history = self.transaction_history.clone();
        let get_tx = move |txid| history.remove(&txid);
        let iter = self.transaction_log.into_iter();
        if reversed {
            Box::new(iter.rev().filter_map(get_tx))
        } else {
            Box::new(iter.filter_map(get_tx))
        }
    }

    /// Returns currently stored unspent transactions for given wallet
    /// if include_pending is true, get all the unspent transactions, else just available transactions
    #[inline]
    pub fn get_unspent_transactions(&self, include_pending: bool) -> BTreeMap<TxoPointer, TxOut> {
        if include_pending {
            self.unspent_transactions.clone()
        } else {
            self.get_available_transactions()
        }
    }

    /// Returns `true` if given transaction inputs are present in the list of unspent transactions, `false` otherwise
    pub fn has_unspent_transactions(&self, inputs: &[TxoPointer]) -> bool {
        let unspent_transactions = self.get_unspent_transactions(false);

        inputs
            .iter()
            .all(|input| unspent_transactions.contains_key(input))
    }
}

/// A memento for wallet state used for batch operations on wallet state service
#[derive(Debug, Default, Clone)]
pub struct WalletStateMemento(Vec<MementoOperation>);

#[derive(Debug, Clone)]
enum MementoOperation {
    AddTransactionChange(TxId, TransactionChange),
    AddUnspentTransaction(TxoPointer, TxOut),
    AddPendingTransaction(TxId, TransactionPending),
    RemovePendingTransaction(TxId),
    RemoveUnspentTransaction(TxoPointer),
}

impl WalletStateMemento {
    /// Adds transaction change to memento
    #[inline]
    pub fn add_transaction_change(&mut self, transaction_change: TransactionChange) {
        self.0.push(MementoOperation::AddTransactionChange(
            transaction_change.transaction_id,
            transaction_change,
        ))
    }

    /// Adds transaction pending info to memento
    #[inline]
    pub fn add_pending_transaction(&mut self, tx_id: TxId, tx_pending: TransactionPending) {
        self.0
            .push(MementoOperation::AddPendingTransaction(tx_id, tx_pending))
    }

    /// Adds unspent transaction to memento
    #[inline]
    pub fn add_unspent_transaction(&mut self, input: TxoPointer, output: TxOut) {
        self.0
            .push(MementoOperation::AddUnspentTransaction(input, output))
    }

    /// Removes unspent transaction from memento
    #[inline]
    pub fn remove_unspent_transaction(&mut self, input: TxoPointer) {
        self.0
            .push(MementoOperation::RemoveUnspentTransaction(input))
    }

    /// Removes pending transaction from memento
    #[inline]
    pub fn remove_pending_transaction(&mut self, tx_id: TxId) {
        self.0
            .push(MementoOperation::RemovePendingTransaction(tx_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secstr::SecUtf8;
    use std::str::FromStr;

    use chain_core::tx::data::address::ExtendedAddr;
    use client_common::tendermint::types::Time;
    use client_common::{seckey::derive_enckey, storage::MemoryStorage};

    use crate::types::{BalanceChange, TransactionType};
    use chain_core::init::coin::Coin;

    #[test]
    fn check_wallet_state_flow() {
        let storage = MemoryStorage::default();
        let name = "name";
        let enckey = &derive_enckey(&SecUtf8::from("passphrase"), name).unwrap();

        let mut wallet_state: WalletState = storage
            .get_value_secure(name, passphrase)
            .unwrap()
            .unwrap_or_default();

        // Check empty state

        assert_eq!(0, wallet_state.get_unspent_transactions(false).len());

        assert_eq!(
            0,
            wallet_state
                .clone()
                .into_transaction_history(false)
                .unwrap()
                .count()
        );

        assert!(wallet_state.get_transaction_change(&[0; 32]).is_none());

        assert_eq!(
            WalletBalance::default(),
            wallet_state.get_balance().unwrap()
        );

        // Add an unspent transaction and check if it is added

        let mut memento = WalletStateMemento::default();
        memento.add_unspent_transaction(
            TxoPointer::new([0; 32], 0),
            TxOut::new(ExtendedAddr::OrTree([0; 32]), Coin::zero()),
        );

        assert!(wallet_state.apply_memento(&memento).is_ok());

        assert_eq!(1, wallet_state.get_unspent_transactions(false).len());

        // Remove previously added unspent transaction and check if it is removed
        let mut memento = WalletStateMemento::default();
        memento.remove_unspent_transaction(TxoPointer::new([0; 32], 0));
        assert!(wallet_state.apply_memento(&memento).is_ok());

        assert_eq!(0, wallet_state.get_unspent_transactions(false).len());

        // Add a pending transaction

        let mut memento = WalletStateMemento::default();
        memento.add_pending_transaction(
            [0; 32],
            TransactionPending {
                used_inputs: vec![],
                block_height: 0,
                return_amount: Coin::unit(),
            },
        );
        assert!(wallet_state.apply_memento(&memento).is_ok());
        // remove the previous added pending transaction
        let mut memento = WalletStateMemento::default();
        memento.remove_pending_transaction([0; 32]);
        assert!(wallet_state.apply_memento(&memento).is_ok());

        storage
            .set_value_secure(name, passphrase, &wallet_state)
            .unwrap();

        let mut wallet_state: WalletState = storage
            .get_value_secure(name, passphrase)
            .unwrap()
            .unwrap_or_default();
        assert_eq!(0, wallet_state.pending_transactions.len());

        // Add a transaction change (with incoming balance) and check if it is added and also new wallet balance

        let mut memento = WalletStateMemento::default();

        memento.add_transaction_change(TransactionChange {
            transaction_id: [0; 32],
            inputs: Vec::new(),
            outputs: Vec::new(),
            balance_change: BalanceChange::Incoming {
                value: Coin::new(50).unwrap(),
            },
            transaction_type: TransactionType::Transfer,
            block_height: 0,
            block_time: Time::from_str("2019-04-09T09:38:41.735577Z").unwrap(),
        });

        assert!(wallet_state.apply_memento(&memento).is_ok());
        assert_eq!(1, wallet_state.get_transaction_history(false).count());
        assert!(wallet_state.get_transaction_change(&[0; 32]).is_some());
        assert!(wallet_state.get_transaction_change(&[1; 32]).is_none());

        // Add a transaction change (with outgoing balance) and check if it is added and also new wallet balance

        let mut memento = WalletStateMemento::default();

        memento.add_transaction_change(TransactionChange {
            transaction_id: [1; 32],
            inputs: Vec::new(),
            outputs: Vec::new(),
            balance_change: BalanceChange::Outgoing {
                value: Coin::new(40).unwrap(),
                fee: Coin::new(10).unwrap(),
            },
            transaction_type: TransactionType::Transfer,
            block_height: 0,
            block_time: Time::from_str("2019-04-09T09:38:41.735577Z").unwrap(),
        });

        assert!(wallet_state.apply_memento(&memento).is_ok());

        assert_eq!(2, wallet_state.get_transaction_history(false).count());

        assert!(wallet_state.get_transaction_change(&[1; 32]).is_some());
    }

    fn prepare_wallet_storage(name: &str, enckey: &SecKey) -> MemoryStorage {
        let storage = MemoryStorage::default();
        let mut wallet_state = WalletState::new();

        let mut memento = WalletStateMemento::default();
        let tx_pointer = |n: u8, i: usize| TxoPointer::new([n; 32], i);
        let output =
            |n: u8, m: u64| TxOut::new(ExtendedAddr::OrTree([n; 32]), Coin::new(m).unwrap());
        // Add two unspent transaction
        memento.add_unspent_transaction(tx_pointer(0, 0), output(0, 100));
        memento.add_unspent_transaction(tx_pointer(0, 1), output(0, 40));
        wallet_state
            .apply_memento(name, passphrase, &memento)
            .unwrap();
        assert_eq!(
            wallet_state.get_balance().unwrap(),
            WalletBalance {
                total: Coin::new(140).unwrap(),
                available: Coin::new(140).unwrap(),
                pending: Coin::zero(),
            }
        );

        // spent the first utxo and return 50 coin
        let mut memento = WalletStateMemento::default();
        memento.add_pending_transaction(
            [1; 32],
            TransactionPending {
                used_inputs: vec![tx_pointer(0, 0)],
                block_height: 1,
                return_amount: Coin::new(50).unwrap(),
            },
        );
        wallet_state.apply_memento(&memento).unwrap();

        assert_eq!(
            wallet_state.get_balance().unwrap(),
            WalletBalance {
                total: Coin::new(90).unwrap(),
                available: Coin::new(40).unwrap(),
                pending: Coin::new(50).unwrap(),
            }
        );

        // now the available utxo is only the second one
        let unspent_tx = wallet_state.get_unspent_transactions(false).unwrap();
        let mut target = BTreeMap::new();
        target.insert(tx_pointer(0, 1), output(0, 40));
        assert_eq!(unspent_tx, target);
        storage
            .set_value_secure(name, passphrase, &wallet_state)
            .unwrap();
        storage
    }

    #[test]
    fn test_sync_and_get_balance() {
        let name = "name";
        let passphrase = &SecUtf8::from("passphrase");
        let storage = prepare_wallet_storage(name, passphrase);
        let mut wallet_state: WalletState = storage.get_value_secure(name, passphrase).unwrap();
        let tx_pointer = |n: u8, i: usize| TxoPointer::new([n; 32], i);
        let output =
            |n: u8, m: u64| TxOut::new(ExtendedAddr::OrTree([n; 32]), Coin::new(m).unwrap());
        let mut memento = WalletStateMemento::default();
        // if the broadcast transaction success, then we should remove the pending transaction and the unspent transaction
        memento.remove_pending_transaction([1; 32]);
        memento.remove_unspent_transaction(tx_pointer(0, 0));
        // and add the returned utxo
        memento.add_unspent_transaction(tx_pointer(1, 0), output(0, 50));
        wallet_state.apply_memento(&memento).unwrap();
        // now, we can get the balance
        assert_eq!(
            wallet_state.get_balance().unwrap(),
            WalletBalance {
                total: Coin::new(90).unwrap(),
                available: Coin::new(90).unwrap(),
                pending: Coin::zero(),
            }
        );
        let unspent_tx = wallet_state.get_unspent_transactions(false);
        assert_eq!(unspent_tx.len(), 2);
    }

    #[test]
    fn test_rollback_and_get_balance() {
        let block_height_ensure = 50;
        let name = "name";
        let passphrase = &SecUtf8::from("passphrase");
        let storage = prepare_wallet_storage(name, passphrase);
        let mut wallet_state: WalletState = storage.get_value_secure(name, passphrase).unwrap();
        // assume that broadcast failed, then we should rollback
        let current_height = 2 + block_height_ensure;
        let rollback_txids =
            wallet_state.get_rollback_pending_tx(current_height, block_height_ensure);
        assert_eq!(rollback_txids, vec![[1; 32]]);
        let mut memento = WalletStateMemento::default();
        for txid in rollback_txids {
            memento.remove_pending_transaction(txid);
        }
        wallet_state.apply_memento(&memento).unwrap();
        assert_eq!(
            wallet_state.get_balance().unwrap(),
            WalletBalance {
                total: Coin::new(140).unwrap(),
                available: Coin::new(140).unwrap(),
                pending: Coin::new(0).unwrap(),
            }
        );
    }
}
