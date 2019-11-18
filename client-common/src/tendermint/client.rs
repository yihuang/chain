use crate::tendermint::types::*;
use crate::Result;

/// Makes remote calls to tendermint (backend agnostic)
pub trait Client: Send + Sync {
    /// Makes `genesis` call to tendermint
    fn genesis(&self) -> Result<Genesis>;

    /// Makes `status` call to tendermint
    fn status(&self) -> Result<Status>;

    /// Makes `block` call to tendermint
    fn block(&self, height: u64) -> Result<Block>;

    /// Makes batched `block` call to tendermint
    fn block_batch<'a, T: Iterator<Item = &'a u64>>(&self, heights: T) -> Result<Vec<Block>>;

    /// Makes `block_results` call to tendermint
    fn block_results(&self, height: u64) -> Result<BlockResults>;

    /// Makes batched `block_results` call to tendermint
    fn block_results_batch<'a, T: Iterator<Item = &'a u64>>(
        &self,
        heights: T,
    ) -> Result<Vec<BlockResults>>;

    /// Make `validators` call to tendermint
    fn validators(&self, _height: u64) -> Result<ValidatorsResponse> {
        unimplemented!()
    }

    /// Make batched `validators` call to tendermint
    fn validators_batch<'a, T: Iterator<Item = &'a u64>>(
        &self,
        _height: T,
    ) -> Result<Vec<ValidatorsResponse>> {
        unimplemented!()
    }

    /// Make `commits` call to tendermint
    fn commit(&self, _height: u64) -> Result<CommitResponse> {
        unimplemented!()
    }

    /// Make batched `commit` call to tendermint
    fn commit_batch<'a, T: Iterator<Item = &'a u64>>(
        &self,
        _height: T,
    ) -> Result<Vec<CommitResponse>> {
        unimplemented!()
    }

    /// Makes `broadcast_tx_sync` call to tendermint
    fn broadcast_transaction(&self, transaction: &[u8]) -> Result<BroadcastTxResponse>;

    /// Makes `abci_query` call to tendermint
    fn query(&self, path: &str, data: &[u8]) -> Result<AbciQuery>;
}
