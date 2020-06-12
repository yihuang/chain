use chain_storage::utxo::{flush_utxo_kvdb, UTxO, UTxOBuffer};
use chain_storage::NUM_COLUMNS;
use rand::Rng;
use std::env;

fn random_utxo() -> UTxO {
    UTxO {
        txid: rand::thread_rng().gen(),
        index: rand::thread_rng().gen(),
    }
}

fn populate(path: &str, count: usize) {
    let store = kvdb_rocksdb::Database::open(
        &kvdb_rocksdb::DatabaseConfig::with_columns(NUM_COLUMNS),
        path,
    )
    .unwrap();
    let utxos = (0..count).map(|_| random_utxo());
    let buffer = utxos.map(|utxo| (utxo, Some(()))).collect::<UTxOBuffer>();
    flush_utxo_kvdb(&store, buffer, 0).unwrap();
}

fn main() {
    let mut args = env::args().skip(1);
    let path = args.next().unwrap();
    let count = args.next().unwrap().parse::<usize>().unwrap();
    println!("{} {}", path, count);
    populate(&path, count);
}
