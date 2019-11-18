#![cfg(feature = "http-rpc")]

use crate::tendermint::lite;
use jsonrpc::client::Client as JsonRpcClient;
use jsonrpc::Request;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::tendermint::types::*;
use crate::tendermint::Client;
use crate::{Error, ErrorKind, Result, ResultExt};

/// Tendermint RPC Client
#[derive(Clone)]
pub struct RpcClient {
    url: String,
}

impl RpcClient {
    /// Creates a new instance of `RpcClient`
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_owned(),
        }
    }

    fn call<T>(&self, name: &str, params: &[Value]) -> Result<T>
    where
        for<'de> T: Deserialize<'de>,
    {
        // jsonrpc does not handle Hyper connection reset properly. The current
        // inefficient workaround is to create a new client on every call.
        // https://github.com/apoelstra/rust-jsonrpc/issues/26
        let client = JsonRpcClient::new(self.url.to_owned(), None, None);
        let request = client.build_request(name, params);
        let response = client.send_request(&request).chain(|| {
            (
                ErrorKind::TendermintRpcError,
                format!(
                    "Unable to make RPC call: Method: {}, Params: {}",
                    name,
                    Value::from(params)
                ),
            )
        })?;
        let result = response.result::<T>().chain(|| {
            (
                ErrorKind::DeserializationError,
                format!(
                    "Unable to deserialize response from RPC method {}: {:?}",
                    name, response
                ),
            )
        })?;
        Ok(result)
    }

    fn call_batch<T>(&self, params: &[(&str, Vec<Value>)]) -> Result<Vec<Option<T>>>
    where
        for<'de> T: Deserialize<'de>,
    {
        if params.is_empty() {
            // Do not send empty batch requests
            return Ok(Default::default());
        }

        if params.len() == 1 {
            // Do not send batch request when there is only one set of params
            self.call::<T>(params[0].0, &params[0].1)
                .map(|value| vec![Some(value)])
        } else {
            // jsonrpc does not handle Hyper connection reset properly. The current
            // inefficient workaround is to create a new client on every call.
            // https://github.com/apoelstra/rust-jsonrpc/issues/26
            let client = JsonRpcClient::new(self.url.to_owned(), None, None);
            let requests = params
                .iter()
                .map(|(name, params)| client.build_request(name, params))
                .collect::<Vec<Request>>();
            let responses = client.send_batch(&requests).chain(|| {
                (
                    ErrorKind::TendermintRpcError,
                    "Unable to make batch RPC call",
                )
            })?;
            responses
                .into_iter()
                .map(|response| -> Result<Option<T>> {
                    response
                        .map(|inner| -> Result<T> {
                            inner.result::<T>().chain(|| {
                                (
                                    ErrorKind::DeserializationError,
                                    format!(
                                        "Unable to deserialize response from batch RPC call: {:?}",
                                        inner,
                                    ),
                                )
                            })
                        })
                        .transpose()
                })
                .collect::<Result<Vec<Option<T>>>>()
        }
    }
}

impl Client for RpcClient {
    fn genesis(&self) -> Result<Genesis> {
        Ok(self
            .call::<GenesisResponse>("genesis", Default::default())?
            .genesis)
    }

    fn status(&self) -> Result<Status> {
        self.call("status", Default::default())
    }

    fn block(&self, height: u64) -> Result<Block> {
        let params = [json!(height.to_string())];
        Ok(self.call::<BlockResponse>("block", &params)?.block)
    }

    fn block_batch<'a, T: Iterator<Item = &'a u64>>(&self, heights: T) -> Result<Vec<Block>> {
        let params = heights
            .map(|height| ("block", vec![json!(height.to_string())]))
            .collect::<Vec<(&str, Vec<Value>)>>();

        let mut rsps = self.call_batch::<BlockResponse>(&params)?;
        rsps.drain(..)
            .map(|rsp| {
                rsp.chain(|| (ErrorKind::InvalidInput, "Block information not found"))
                    .map(|rsp_| rsp_.block)
            })
            .collect::<Result<Vec<Block>>>()
    }

    fn block_results(&self, height: u64) -> Result<BlockResults> {
        let params = [json!(height.to_string())];
        self.call("block_results", &params)
    }

    fn block_results_batch<'a, T: Iterator<Item = &'a u64>>(
        &self,
        heights: T,
    ) -> Result<Vec<BlockResults>> {
        let params = heights
            .map(|height| ("block_results", vec![json!(height.to_string())]))
            .collect::<Vec<(&str, Vec<Value>)>>();
        let response = self.call_batch::<BlockResults>(&params)?;

        response
            .into_iter()
            .map(|block_results| {
                block_results.chain(|| {
                    (
                        ErrorKind::InvalidInput,
                        "Block results information not found",
                    )
                })
            })
            .collect::<Result<Vec<BlockResults>>>()
    }

    fn validators(&self, height: u64) -> Result<ValidatorsResponse> {
        let params = [json!(height.to_string())];
        self.call("validators", &params)
    }

    fn validators_batch<'a, T: Iterator<Item = &'a u64>>(
        &self,
        heights: T,
    ) -> Result<Vec<ValidatorsResponse>> {
        let params = heights
            .map(|height| ("validators", vec![json!(height.to_string())]))
            .collect::<Vec<(&str, Vec<Value>)>>();
        let rsps = self.call_batch::<ValidatorsResponse>(&params)?;

        rsps.into_iter()
            .map(|rsp| rsp.chain(|| (ErrorKind::InvalidInput, "Validators information not found")))
            .collect::<Result<Vec<ValidatorsResponse>>>()
    }

    fn commit(&self, height: u64) -> Result<CommitResponse> {
        let params = [json!(height.to_string())];
        self.call("commit", &params)
    }

    fn commit_batch<'a, T: Iterator<Item = &'a u64>>(
        &self,
        heights: T,
    ) -> Result<Vec<CommitResponse>> {
        let params = heights
            .map(|height| ("commit", vec![json!(height.to_string())]))
            .collect::<Vec<(&str, Vec<Value>)>>();
        let rsps = self.call_batch::<CommitResponse>(&params)?;

        rsps.into_iter()
            .map(|rsp| rsp.chain(|| (ErrorKind::InvalidInput, "Validators information not found")))
            .collect::<Result<Vec<CommitResponse>>>()
    }

    fn block_batch_verified(
        &self,
        state: lite::TrustedState,
        min_height: u64,
        max_height: u64,
    ) -> Result<Vec<Block>> {
        Ok(vec![])
    }

    fn broadcast_transaction(&self, transaction: &[u8]) -> Result<BroadcastTxResponse> {
        let params = [json!(transaction)];
        self.call::<BroadcastTxResponse>("broadcast_tx_sync", &params)
            .and_then(|result| {
                if result.code.is_err() {
                    Err(Error::new(
                        ErrorKind::TendermintRpcError,
                        result.log.to_string(),
                    ))
                } else {
                    Ok(result)
                }
            })
    }

    fn query(&self, path: &str, data: &[u8]) -> Result<AbciQuery> {
        let params = [
            json!(path),
            json!(hex::encode(data)),
            json!(null),
            json!(null),
        ];
        let result = self
            .call::<AbciQueryResponse>("abci_query", &params)?
            .response;

        if result.code.is_err() {
            return Err(Error::new(
                ErrorKind::TendermintRpcError,
                result.log.to_string(),
            ));
        }

        Ok(result)
    }
}
