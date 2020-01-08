#![cfg_attr(all(feature = "mesalock_sgx", not(target_env = "sgx")), no_std)]
#![cfg_attr(target_env = "sgx", feature(rustc_private))]

#[cfg(all(feature = "mesalock_sgx", not(target_env = "sgx")))]
extern crate sgx_tstd as std;

use chain_core::state::account::WithdrawUnbondedTx;
use chain_core::tx::data::access::TxAccessPolicy;
use chain_core::tx::data::attribute::TxAttributes;
use chain_core::tx::data::Tx;
use chain_core::tx::data::TxId;
use chain_core::tx::TxWithOutputs;
use parity_scale_codec::Decode;
use secp256k1::key::PublicKey;
use sgx_wrapper::{SealedData, Vec};

#[inline]
fn is_allowed_view(
    allowed_views: &[TxAccessPolicy],
    view_key: &Option<PublicKey>,
    check_allowed_views: bool,
) -> bool {
    match view_key {
        Some(view_key) if check_allowed_views => {
            // TODO: policy != alldata + const eq?
            allowed_views.iter().any(|x| x.view_key == *view_key)
        }
        _ => !check_allowed_views,
    }
}

/// A helper function to unseal a vector of transactions with outputs
/// and check each transaction against the expected transaction id
/// + does an optional view key check.
/// If something went wrong (e.g. wrong data was passed in), it'll return None.
/// If OK, it'll return Some(vector of transactions -- optionally only ones that match the view key)
///
/// Use case #1: transaction validation
/// (view key should be None+check_allowed_views should be false, as that TX data won't be returned in plain)
/// Use case #2: transaction querying
/// (assuming view key signature was checked before in the decryption request
/// -- view key should be Some(vk) + check_allowed_views should be true;
/// only returns transactions where the view key is included)
#[inline]
pub fn check_unseal<I>(
    view_key: Option<PublicKey>,
    check_allowed_views: bool,
    txids: I,
    mut sealed_logs: Vec<Vec<u8>>,
) -> Option<Vec<TxWithOutputs>>
where
    I: IntoIterator<Item = TxId> + ExactSizeIterator,
{
    let mut return_result = Vec::with_capacity(sealed_logs.len());
    for (txid, mut sealed_log) in txids.into_iter().zip(sealed_logs.iter_mut()) {
        let sealed_data = match SealedData::from_bytes(&mut sealed_log) {
            Some(data) => data,
            None => {
                return None;
            }
        };
        let result = sealed_data.unseal_data();
        let mut unsealed_data = match result {
            Ok(x) => x,
            Err(_) => {
                return None;
            }
        };
        if unsealed_data.get_additional_txt() != txid {
            unsealed_data.clear();
            return None;
        }
        let otx = TxWithOutputs::decode(&mut unsealed_data.get_decrypt_txt());
        let push: bool;
        match &otx {
            Ok(TxWithOutputs::Transfer(Tx {
                attributes: TxAttributes { allowed_view, .. },
                ..
            })) => {
                push = is_allowed_view(&allowed_view, &view_key, check_allowed_views);
            }
            Ok(TxWithOutputs::StakeWithdraw(WithdrawUnbondedTx {
                attributes: TxAttributes { allowed_view, .. },
                ..
            })) => {
                push = is_allowed_view(&allowed_view, &view_key, check_allowed_views);
            }
            _ => {
                unsealed_data.clear();
                return None;
            }
        }
        if push {
            return_result.push(otx.unwrap());
        }
        unsealed_data.clear();
    }
    Some(return_result)
}
