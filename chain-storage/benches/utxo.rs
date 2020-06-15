use chain_storage::buffer::Get;
use chain_storage::utxo::{flush_utxo_kvdb, UTxO, UTxOBuffer, UTxOGetter};
use chain_storage::NUM_COLUMNS;
use criterion::{criterion_group, criterion_main, Criterion};
use rand::Rng;

const DB_PATH: &str = "/data/yihuang/chain/tmpdb";
const VERSION: u64 = 10;

fn random_utxo() -> UTxO {
    UTxO {
        txid: rand::thread_rng().gen(),
        index: rand::thread_rng().gen(),
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    let store = kvdb_rocksdb::Database::open(
        &kvdb_rocksdb::DatabaseConfig::with_columns(NUM_COLUMNS),
        DB_PATH,
    )
    .unwrap();

    c.bench_function("jellyfish-utxo, get", |b| {
        let utxo = random_utxo();
        b.iter(|| {
            UTxOGetter::new(&store, VERSION - 2).get(&utxo);
        });
    });

    c.bench_function("jellyfish-utxo, remove", |b| {
        let utxo = random_utxo();
        let buffer = vec![(utxo, None)].into_iter().collect::<UTxOBuffer>();
        b.iter(|| {
            flush_utxo_kvdb(&store, buffer.clone(), VERSION).unwrap();
        });
    });
    c.bench_function("jellyfish-utxo, insert", |b| {
        let utxo = random_utxo();
        let buffer = vec![(utxo, Some(()))].into_iter().collect::<UTxOBuffer>();
        b.iter(|| {
            flush_utxo_kvdb(&store, buffer.clone(), VERSION).unwrap();
        });
    });
    // c.bench_function("jellyfish-utxo, insert256", |b| {
    //     let utxos = (0..256).map(|_| random_utxo());
    //     let buffer = utxos
    //         .map(|utxo| (utxo, Some(())))
    //         .into_iter()
    //         .collect::<UTxOBuffer>();
    //     b.iter(|| {
    //         flush_utxo_kvdb(&store, buffer.clone(), VERSION).unwrap();
    //     });
    // });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
