//! Lite tendermint client
use serde::{Deserialize, Serialize};
use tendermint::{block::Header, block::Height, lite::verifier, validator, Block};

use crate::tendermint::client::Client;
use crate::{Error, ErrorKind, Result};

///
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TrustedState {
    /// last header
    pub header: Option<Header>,
    /// current validator set
    pub validators: validator::Set,
}

impl TrustedState {
    /// load TrustedState from trusted height
    pub fn from_trusted_height<C>(client: &C, height: Height) -> Result<TrustedState>
    where
        C: Client,
    {
        Ok(TrustedState {
            header: Some(client.block(height.value())?.header),
            validators: get_validator_set(client, height)?,
        })
    }
}

/// get validator set from rpc
pub fn get_validator_set<C>(client: &C, height: Height) -> Result<validator::Set>
where
    C: Client,
{
    Ok(validator::Set::new(
        client.validators(height.value())?.validators,
    ))
}

/// get genesis validator set
pub fn get_genesis_validators<C>(client: &C) -> Result<validator::Set>
where
    C: Client,
{
    Ok(validator::Set::new(client.genesis()?.validators))
}

/// get block and verify
pub fn get_verified_block<C>(client: &C, height: Height, state: &mut TrustedState) -> Result<Block>
where
    C: Client,
{
    let block = client.block(height.value())?;
    let commit = client.commit(height.value())?;
    let next_validators = validator::Set::new(client.validators(height.value())?.validators);
    verifier::verify_trusting(
        block.header.clone(),
        commit.signed_header,
        state.validators.clone(),
        next_validators.clone(),
    )
    .map_err(|err| {
        Error::new(
            ErrorKind::VerifyError,
            format!("block verify failed: {:?}", err),
        )
    })?;
    state.header = Some(block.header.clone());
    state.validators = next_validators;
    Ok(block)
}
