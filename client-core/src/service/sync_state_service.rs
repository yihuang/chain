use parity_scale_codec::{Decode, Encode};
use tendermint::validator;

use client_common::tendermint::lite;
use client_common::StorageValueType;

/// Sync state for wallet
#[derive(Debug, Encode, Decode)]
pub struct SyncState {
    /// last block height
    pub last_block_height: u64,
    /// last app hash
    pub last_app_hash: String,
    /// current trusted state for lite client verification
    pub trusted_state: lite::TrustedState,
}

impl StorageValueType for SyncState {
    #[inline]
    fn keyspace() -> &'static str {
        "core_wallet_sync"
    }
}

impl SyncState {
    /// construct genesis global state
    pub fn genesis(genesis_validators: Vec<validator::Info>) -> SyncState {
        SyncState {
            last_block_height: 0,
            last_app_hash: "".to_owned(),
            trusted_state: lite::TrustedState::genesis(genesis_validators),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json;

    use super::*;
    use client_common::{storage::MemoryStorage, StorageValueType};

    #[test]
    fn check_sync_state_serialization() {
        let trusted_state_json = r#"{"header":{"version":{"block":"10","app":"0"},"chain_id":"test-chain-y3m1e6-AB","height":"1","time":"2019-11-20T08:56:48.618137Z","num_txs":"0","total_txs":"0","last_block_id":null,"last_commit_hash":null,"data_hash":null,"validators_hash":"1D19568662F9A9167B338F98C860C4102AA0DE85600BF48A15B192DB53D030A1","next_validators_hash":"1D19568662F9A9167B338F98C860C4102AA0DE85600BF48A15B192DB53D030A1","consensus_hash":"048091BC7DDC283F77BFBF91D73C44DA58C3DF8A9CBC867405D8B7F3DAADA22F","app_hash":"0F46E113C21F9EACB26D752F9523746CF8D47ECBEA492736D176005911F973A5","last_results_hash":null,"evidence_hash":null,"proposer_address":"A59B92278703DFECE52A40D9EF3AE9D1EDC6B949"},"validators":{"validators":[{"address":"A59B92278703DFECE52A40D9EF3AE9D1EDC6B949","pub_key":{"type":"tendermint/PubKeyEd25519","value":"oblY1MjCzNuYlr7A5cUsEY3yBxYBSRHzha16wbnWNx8="},"voting_power":"5000000000","proposer_priority":"0"}]}}"#;
        let mut state = SyncState::genesis(vec![]);
        state.last_block_height = 1;
        state.last_app_hash =
            "0F46E113C21F9EACB26D752F9523746CF8D47ECBEA492736D176005911F973A5".to_owned();
        state.trusted_state = serde_json::from_str(trusted_state_json).unwrap();

        let key = "Default";

        let storage = MemoryStorage::default();
        storage.set_value(key, &state).unwrap();
        let state1: SyncState = storage.get_value(key).unwrap().unwrap();

        assert_eq!(state.encode(), state1.encode());
    }
}
