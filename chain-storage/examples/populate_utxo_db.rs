use chain_storage::utxo::{flush_utxo_kvdb, UTxO, UTxOBuffer};
use chain_storage::NUM_COLUMNS;
use rand::Rng;
use std::env;
use std::time::SystemTime;

fn random_utxo() -> UTxO {
    UTxO {
        txid: rand::thread_rng().gen(),
        index: rand::thread_rng().gen(),
    }
}

fn populate(path: &str, count: usize, version: u64) {
    println!("begin: {:?}", std::time::SystemTime::now());

    let store = kvdb_rocksdb::Database::open(
        &kvdb_rocksdb::DatabaseConfig::with_columns(NUM_COLUMNS),
        path,
    )
    .unwrap();
    let utxos = (0..count).map(|_| random_utxo());
    let buffer = utxos.map(|utxo| (utxo, Some(()))).collect::<UTxOBuffer>();
    println!("prepare data: {:?}", std::time::SystemTime::now());
    flush_utxo_kvdb(&store, buffer, version).unwrap();
    println!("after flush: {:?}", std::time::SystemTime::now());
}

fn main() {
    let mut args = env::args().skip(1);
    let path = args.next().unwrap();
    let count = args.next().unwrap().parse::<usize>().unwrap();
    let version = args.next().unwrap().parse::<u64>().unwrap();
    println!("{} {} {}", path, count, version);
    populate(&path, count, version);
}
