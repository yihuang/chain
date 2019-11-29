use chain_core::common::{Timespec, H256};
use chain_core::init::config::NetworkParameters;
use chain_core::state::account::CouncilNode;
use chain_core::state::tendermint::BlockHeight;
use chain_core::state::RewardsPoolState;

use super::storage::tx::StarlingFixedKey;
use super::validator::ValidatorSet;

/// block header or genesis infomation
struct Header {
    app_hash: H256,
    /// zero for genesis.
    height: BlockHeight,
    time: Timespec,
}

impl Header {
    fn is_genesis(&self) -> bool {
        self.height == 0
    }
}

struct ChainState {
    header: Header,
    account_root: StarlingFixedKey,
    council_nodes: Vec<CouncilNode>,
}
