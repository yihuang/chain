use serde_json;

use client_common::tendermint::{lite, Client};
use client_common::{ErrorKind, Result, ResultExt, Storage};
use tendermint::{block::Height, validator};

const KEYSPACE: &str = "verified_sync";
const STATE_KEY: &str = "state";

///
pub struct VerifiedSynchronizer<S, C>
where
    S: Storage,
    C: Client,
    // H: BlockHandler,
{
    storage: S,
    client: C,
    // block_handler: H,
}

impl<S, C> VerifiedSynchronizer<S, C>
where
    S: Storage,
    C: Client,
    // H: BlockHandler,
{
    ///
    pub fn new(storage: S, client: C) -> Self {
        Self { storage, client }
    }

    /// sync blocks
    pub fn sync(&self, _height: Option<Height>) -> Result<()> {
        let mut state = self.load_state()?;
        println!("state: {}", serde_json::to_string(&state).unwrap());

        let next_height = state
            .header
            .as_ref()
            .map_or(Height::default().increment(), |header| {
                header.height.increment()
            });

        let _block = lite::get_verified_block(&self.client, next_height, &mut state)?;
        // self.block_handler.on_next("", SecUtf8::from_str(""), block);
        // save state
        self.save_state(&state)
    }

    fn load_state(&self) -> Result<lite::TrustedState> {
        // get current trusted state
        let opt = self.storage.get(KEYSPACE, STATE_KEY)?;
        match opt {
            None => Ok(lite::TrustedState {
                header: None,
                validators: validator::Set::new(self.client.genesis()?.validators),
            }),
            Some(bytes) => serde_json::from_slice(bytes.as_slice()).chain(|| {
                (
                    ErrorKind::DeserializationError,
                    format!(
                        "Unable to deserialize trusted state from storage: {:?}",
                        bytes
                    ),
                )
            }),
        }
    }

    fn save_state(&self, state: &lite::TrustedState) -> Result<()> {
        let s = serde_json::to_string(state).chain(|| {
            (
                ErrorKind::DeserializationError,
                "Unable to serialize trusted state",
            )
        })?;

        let _result = self
            .storage
            .set(KEYSPACE, STATE_KEY, s.as_bytes().iter().cloned().collect());
        Ok(())
    }
}
