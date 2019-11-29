use chain_core::common::{MerkleTree, Timespec, H256};
use chain_core::compute_app_hash;
use chain_core::init::config::SlashRatio;
use chain_core::init::{
    config::GenesisState, config::NetworkParameters, params::InitNetworkParameters,
};
use chain_core::state::account::{to_stake_key, CouncilNode, StakedState, StakedStateAddress};
use chain_core::state::tendermint::{BlockHeight, TendermintValidatorAddress, TendermintVotePower};
use chain_core::state::RewardsPoolState;
use chain_core::tx::fee::Milli;
use chain_core::tx::TxAux;

use super::app::compute_accounts_root;
use super::enclave_bridge::EnclaveProxy;
use super::storage::account::{AccountStorage, AccountWrapper};
use super::storage::tx::StarlingFixedKey;
use super::validator::{Validator, ValidatorCandidate, ValidatorSet};

#[derive(Debug, Clone)]
struct AccountRoot(StarlingFixedKey);

impl AccountRoot {
    fn new(root: StarlingFixedKey) -> AccountRoot {
        AccountRoot(root)
    }

    fn get(&self, storage: &AccountStorage, addr: &StakedStateAddress) -> Option<StakedState> {
        storage
            .get_one(&self.0, &to_stake_key(addr))
            .expect("account storage io error")
            .map(|wapper| wapper.0)
    }
    fn update(&mut self, storage: &mut AccountStorage, value: &StakedState) {
        self.0 = storage
            .insert_one(
                Some(&self.0),
                &value.key(),
                &AccountWrapper(value.clone()), // FIXME remove the clone
            )
            .expect("update account");
    }
}

/// block/genesis meta infomation
/// TODO a better type name for the fact it can also represent genesis?
#[derive(Debug, Clone)]
struct Header {
    /// zero for genesis.
    height: BlockHeight,
    app_hash: H256,
    time: Timespec,
}

impl Header {
    fn from_genesis(app_hash: H256, time: Timespec) -> Header {
        Header {
            app_hash,
            time,
            height: 0,
        }
    }
}

/// Temporary state for deliver_tx before commit.
struct TxDeliverState {
    header: Header, // current uncommited block header
    delivered_txs: Vec<TxAux>,
    account_root: AccountRoot,      // cloned from last commited state.
    rewards_pool: RewardsPoolState, // cloned from last commited state.
    validators: ValidatorSet,       // cloned from last commited state.
}

impl TxDeliverState {
    fn from_last_state(state: &ChainState) -> TxDeliverState {
        TxDeliverState {
            header: state.header.clone(),
            delivered_txs: vec![],
            account_root: state.account_root,
            rewards_pool: state.rewards_pool.clone(),
            validators: state.validators.clone(),
        }
    }
}

/// Last commited chain state
struct ChainState {
    header: Header, // last commited haeder
    account_root: AccountRoot,
    rewards_pool: RewardsPoolState,
    network_params: NetworkParameters,
    validators: ValidatorSet,
}

impl ChainState {
    fn iter_candidates(&self) -> impl Iterator<Item = &Validator> {
        self.validators.iter_candidates(
            self.network_params.get_required_council_node_stake(),
            self.network_params.get_max_validators(),
        )
    }
}

struct App<T: EnclaveProxy> {
    // external services
    tx_enclave: T,
    account_storage: AccountStorage,
    tx_query_address: Option<String>,

    state: Option<ChainState>,
    tx_deliver_state: Option<TxDeliverState>,
}

/// FIXME directly include `Vec<Validator>` in GenesisState
fn validators_from_genesis_state(
    nodes: Vec<(StakedStateAddress, CouncilNode)>,
    accounts: &[StakedState],
    block_signing_window: u16,
) -> ValidatorSet {
    nodes
        .into_iter()
        .map(|(addr, node)| {
            let index = accounts
                .iter()
                .position(|acct| acct.address == addr)
                .expect("invalid genesis");
            Validator::new(node, addr, accounts[index].unbonded, block_signing_window)
        })
        .collect()
}

// fn slashing_proportion<I: Iterator<Item = TendermintVotePower>>(
//     slash_powers: I,
//     total: TendermintVotePower,
// ) -> SlashRatio {
//     let slashing_proportion = Milli::from_millis(
//         slash_powers
//             .map(|power| (Milli::new(power.into(), 0) / total).sqrt().as_millis())
//             .sum(),
//     );
//
//     std::cmp::min(Milli::new(1, 0), slashing_proportion * slashing_proportion)
//         .try_into()
//         .unwrap() // This will never panic because input is always lower than 1.0
// }

impl<T: EnclaveProxy> App<T> {
    fn new(tx_enclave: T, account_storage: AccountStorage) -> App<T> {
        App {
            tx_enclave,
            account_storage,
            tx_query_address: None,
            state: None,
            tx_deliver_state: None,
        }
    }
    fn init_chain(
        &mut self,
        init_state: GenesisState,
        init_params: InitNetworkParameters,
        genesis_time: Timespec,
    ) -> Vec<ValidatorCandidate> {
        assert!(self.state.is_none());
        let (accounts, rewards_pool, nodes) = init_state;
        let network_params = NetworkParameters::Genesis(init_params);
        let account_root = compute_accounts_root(&mut self.account_storage, &accounts);
        let app_hash = compute_app_hash(
            &MerkleTree::empty(),
            &account_root,
            &rewards_pool,
            &network_params,
        );
        let header = Header::from_genesis(app_hash, genesis_time);
        let validators = validators_from_genesis_state(
            nodes,
            &accounts,
            network_params.get_block_signing_window(),
        );
        let state = ChainState {
            header,
            account_root,
            rewards_pool,
            network_params,
            validators,
        };
        let candidates = state.iter_candidates().map(|v| v.into()).collect();
        self.state = Some(state);
        candidates
    }
    fn begin_block(
        &mut self,
        header: Header,
        last_commit_info: &[(StakedStateAddress, bool)],
        byzantine_validators: &[TendermintValidatorAddress],
    ) {
        let state = self.state.as_mut().expect("chain not initialized");

        // update liveness
        // if header.height > 1 {
        //     state
        //         .validators
        //         .update_livenesses(header.height - 1, last_commit_info);
        // }

        // state.validators.punish_unlive_validators();
        // punish byzantine validators
        let total_power = state.iter_candidates().map(|v| v.voting_power()).sum();
        // for addr in byzantine_validators {
        //     state
        //         .validators
        //         .slash(addr, SlashingSchedule::new(
        //                 state.network_params.get_byzantine_slash_percent() * ))
        //         .unwrap()
        // }

        self.tx_deliver_state = Some(TxDeliverState::from_last_state(state));
    }
    // fn deliver_tx(&mut self, txaux: TxAux) -> Events {}
    fn end_block(&mut self) {}
    fn commit(&mut self) {}
}
