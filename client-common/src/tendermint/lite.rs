//! Lite tendermint client
use std::time::{Duration, SystemTime};

use parity_scale_codec::{Decode, Encode, Error, Input, Output};
use serde::{Deserialize, Serialize};
use serde_json;
use tendermint::{
    block::signed_header::SignedHeader,
    block::Header,
    lite,
    lite::types::TrustedState as _,
    lite::types::{Header as _, ValidatorSet as _},
    validator,
};

use crate::tendermint::client::Client;
use crate::{Error as CommonError, ErrorKind, Result as CommonResult};

///
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TrustedState {
    /// last header
    header: Option<Header>,
    /// current validator set
    validators: validator::Set,
}

impl lite::TrustedState for TrustedState {
    type Header = Header;
    type ValidatorSet = validator::Set;
    fn last_header(&self) -> Option<&Header> {
        self.header.as_ref()
    }
    fn validators(&self) -> &validator::Set {
        &self.validators
    }
    fn new(last_header: Option<Header>, validators: validator::Set) -> TrustedState {
        TrustedState {
            header: last_header,
            validators,
        }
    }
}

impl TrustedState {
    /// Construct genesis trusted state
    pub fn genesis(vals: Vec<validator::Info>) -> TrustedState {
        TrustedState::new(None, validator::Set::new(vals))
    }
}

impl Encode for TrustedState {
    fn encode_to<T: Output>(&self, dest: &mut T) {
        serde_json::to_string(self).unwrap().encode_to(dest)
    }
}

impl Decode for TrustedState {
    fn decode<I: Input>(value: &mut I) -> Result<Self, Error> {
        serde_json::from_str(&String::decode(value)?)
            .map_err(|_| "fail to decode trusted_state from json ".into())
    }
}

/// get genesis validator set
pub fn get_genesis_validators<C>(client: &C) -> CommonResult<validator::Set>
where
    C: Client,
{
    Ok(validator::Set::new(client.genesis()?.validators))
}

/// Default trust level
pub struct DefaultTrustLevel();
impl lite::TrustThreshold for DefaultTrustLevel {}

/// Verify new header against trusted state
pub fn verify_new_header(
    state: &TrustedState,
    signed_header: &SignedHeader,
    next_vals: &validator::Set,
) -> CommonResult<TrustedState> {
    println!(
        "current validators: {} {}",
        state.validators().hash(),
        signed_header.header.validators_hash()
    );
    println!(
        "next validators: {} {} {}",
        signed_header.header.height(),
        signed_header.header.next_validators_hash(),
        next_vals.hash(),
    );
    lite::verify_new_header(
        state,
        signed_header,
        next_vals,
        DefaultTrustLevel(),
        Duration::from_secs(1),
        SystemTime::now(),
    )
    .map_err(|err| {
        CommonError::new_with_source(ErrorKind::VerifyError, "verify new header", err.into())
    })
}
