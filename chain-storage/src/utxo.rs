use std::collections::HashMap;

use anyhow::Result;
use jellyfish_merkle::{AccountStateBlob, HashValue, JellyfishMerkleTree, Version};
use kvdb::KeyValueDB;
use parity_scale_codec::{Decode, Encode};

use crate::{
    buffer::{BufferGetter, BufferStore, Get, GetKV, Store, StoreKV},
    jellyfish::{encode_stale_node_index, KVReader},
    COL_TRIE_NODE, COL_TRIE_STALED,
};
use chain_core::common::H256;

pub type UTxOBuffer = HashMap<UTxO, Option<()>>;

/// Specialized for staking
pub trait StoreUTxO: Store<Key = UTxO, Value = ()> {}
impl<S> StoreUTxO for S where S: Store<Key = UTxO, Value = ()> {}

pub struct UTxOGetter<'a, S: GetKV> {
    storage: &'a S,
    version: Version,
}

impl<'a, S: GetKV> UTxOGetter<'a, S> {
    pub fn new(storage: &'a S, version: Version) -> Self {
        Self { storage, version }
    }
}

impl<'a, S: GetKV> Get for UTxOGetter<'a, S> {
    type Key = UTxO;
    type Value = ();
    fn get(&self, key: &Self::Key) -> Option<Self::Value> {
        JellyfishMerkleTree::new(&KVReader::new(self.storage))
            .get_with_proof(key.hash(), self.version)
            .expect("merkle trie internal error")
            .0
            .map(|_| ())
    }
}

/// Specialized for utxo
pub type UTxOBufferStore<'a, S, H> = BufferStore<'a, UTxOGetter<'a, S>, H>;
/// Specialized for utxo
pub type UTxOBufferGetter<'a, S, H> = BufferGetter<'a, UTxOGetter<'a, S>, H>;

#[derive(Clone, Encode, Decode, PartialEq, Eq, Hash)]
pub struct UTxO {
    pub txid: [u8; 32],
    pub index: u16,
}

impl UTxO {
    pub fn hash(&self) -> HashValue {
        HashValue::new(blake3::hash(&self.encode()).into())
    }

    pub fn blob(&self) -> (HashValue, Option<AccountStateBlob>) {
        let bytes = self.encode();
        let hash = HashValue::new(blake3::hash(&bytes).into());
        (hash, Some(bytes.into()))
    }
}

fn utxo_blob(utxo: &UTxO, value: Option<()>) -> (HashValue, Option<AccountStateBlob>) {
    let bytes = utxo.encode();
    let hash = HashValue::new(blake3::hash(&bytes).into());
    (hash, value.map(|_| bytes.into()))
}

pub fn flush_utxo(
    storage: &mut impl StoreKV,
    buffer: UTxOBuffer,
    version: Version,
) -> Result<H256> {
    let reader = KVReader::new(&*storage);
    let tree = JellyfishMerkleTree::new(&reader);
    let blobs = buffer
        .into_iter()
        .map(|(k, v)| utxo_blob(&k, v))
        .collect::<Vec<_>>();
    let (root_hashes, batch) = tree.put_blob_sets2(vec![blobs], version).unwrap();
    for (key, node) in batch.node_batch.iter() {
        storage.set(
            (COL_TRIE_NODE, key.encode().unwrap()),
            node.encode().unwrap(),
        );
    }
    for key in batch.stale_node_index_batch {
        storage.set((COL_TRIE_STALED, encode_stale_node_index(&key)?), vec![]);
    }
    Ok(*root_hashes[0].as_ref())
}

pub fn flush_utxo_kvdb(
    storage: &impl KeyValueDB,
    buffer: UTxOBuffer,
    version: Version,
) -> Result<H256> {
    let mut tx = storage.transaction();

    let reader = KVReader::new(&*storage);
    let tree = JellyfishMerkleTree::new(&reader);
    let blobs = buffer
        .into_iter()
        .map(|(k, v)| utxo_blob(&k, v))
        .collect::<Vec<_>>();
    let (root_hashes, batch) = tree.put_blob_sets2(vec![blobs], version).unwrap();
    for (key, node) in batch.node_batch.iter() {
        tx.put(COL_TRIE_NODE, &key.encode()?, &node.encode()?);
    }
    for key in batch.stale_node_index_batch {
        tx.put(COL_TRIE_STALED, &encode_stale_node_index(&key)?, &[]);
    }
    storage.write(tx)?;
    Ok(*root_hashes[0].as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jellyfish::collect_stale_nodes;
    use crate::NUM_COLUMNS;
    use kvdb_memorydb::create as create_memorydb;
    use rand::Rng;

    fn random_utxo() -> UTxO {
        UTxO {
            txid: rand::thread_rng().gen(),
            index: rand::thread_rng().gen(),
        }
    }

    #[test]
    fn check_utxo() {
        let storage = create_memorydb(NUM_COLUMNS);
        let mut buffer = UTxOBuffer::new();
        let utxo1 = UTxO::random();
        let utxo2 = UTxO::random();
        let utxo3 = UTxO::random();
        // insert
        buffer.insert(utxo1.clone(), Some(()));
        buffer.insert(utxo2.clone(), Some(()));
        buffer.insert(utxo3.clone(), Some(()));
        let _root = flush_utxo_kvdb(&storage, std::mem::take(&mut buffer), 0).unwrap();
        // delete
        buffer.insert(utxo1.clone(), None);
        buffer.insert(utxo2.clone(), None);
        buffer.insert(utxo3.clone(), None);
        let _root = flush_utxo_kvdb(&storage, std::mem::take(&mut buffer), 1).unwrap();

        // only an empty root node left
        let stale_count = collect_stale_nodes(&storage, 1).len();
        assert_eq!(storage.iter(COL_TRIE_NODE).count() - stale_count, 1);
    }
}
