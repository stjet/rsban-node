use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use rsnano_core::{utils::ConsoleLogger, Account};
use rsnano_ledger::{Ledger, LedgerConstants, WriteDatabaseQueue};
use rsnano_node::cementation::{BlockCementer, CementCallbacks};
use rsnano_store_lmdb::LmdbStore;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let Ok(mode) = get_mode(&args) else {
        eprintln!("usage: cementation_test [clear] path/to/data.ldb");
        return;
    };

    match mode {
        Mode::Clear(ledger_path) => {
            println!("Clearing confirmation height at {:?}", ledger_path);
            let ledger = open_ledger(ledger_path);
            let mut txn = ledger.rw_txn();
            let started = Instant::now();
            ledger.store.confirmation_height().clear(txn.as_mut());
            txn.commit();
            println!("clearing took {} ms", started.elapsed().as_millis());
        }
        Mode::Cement(ledger_path) => {
            println!("Running test with ledger {:?}", ledger_path);
            let ledger = Arc::new(open_ledger(ledger_path));

            let write_queue = Arc::new(WriteDatabaseQueue::new(true));
            let logger = Arc::new(ConsoleLogger::new());

            let mut cementer = BlockCementer::new(
                ledger.clone(),
                write_queue,
                logger,
                true,
                Duration::from_millis(250),
                Arc::new(AtomicBool::new(false)),
            );

            let txn = ledger.read_txn();
            let mut block_queue = VecDeque::new();

            let mut iterator = ledger
                .store
                .account()
                .begin_account(txn.txn(), &Account::from(0));

            while let Some((account, info)) = iterator.current() {
                let conf_height = ledger
                    .store
                    .confirmation_height()
                    .get(txn.txn(), account)
                    .unwrap_or_default();

                if conf_height.height != info.block_count {
                    let head = ledger.store.block().get(txn.txn(), &info.head).unwrap();
                    block_queue.push_back(head);
                    if block_queue.len() == 300_000 {
                        break;
                    }
                }

                iterator.next();
            }
            println!("Added all blocks!");

            let mut processed_count = 0;
            let cementation_start = Instant::now();
            let mut measure_start = Instant::now();
            let mut measure_counter = ledger.cache.cemented_count.load(Ordering::Relaxed);

            while let Some(block) = block_queue.pop_front() {
                let awaiting = block_queue.len() as u64;
                let mut callbacks = CementCallbacks {
                    block_cemented: Box::new(|_| {}),
                    block_already_cemented: Box::new(|_| {}),
                    awaiting_processing_count: Box::new(move || awaiting),
                };

                cementer.process(&block, &mut callbacks.as_refs());
                processed_count += 1;

                if processed_count % 100 == 0 {
                    let old_start = measure_start;
                    let old_count = measure_counter;
                    measure_start = Instant::now();
                    measure_counter = ledger.cache.cemented_count.load(Ordering::Relaxed);
                    let rate = if measure_start - old_start < Duration::from_millis(100) {
                        0
                    } else {
                        ((measure_counter - old_count) as f64
                            / (measure_start - old_start).as_secs_f64())
                            as usize
                    };

                    println!(
                        "{} blocks cemented. Cementation queue len is {}. Cementing at {} blocks/s",
                        ledger.cache.cemented_count.load(Ordering::Relaxed),
                        block_queue.len(),
                        rate
                    );
                }
            }

            println!(
                "all blocks cemented after {:?}",
                cementation_start.elapsed()
            );
        }
    }
}

fn open_ledger<T: AsRef<Path>>(ledger_path: T) -> Ledger {
    let store = Arc::new(LmdbStore::open(ledger_path.as_ref()).build().unwrap());
    Ledger::new(store, LedgerConstants::beta().unwrap()).unwrap()
}

enum Mode {
    Clear(PathBuf),
    Cement(PathBuf),
}

fn get_mode(args: &[String]) -> Result<Mode, ()> {
    if args.len() == 2 || args.len() == 3 {
        if args[1] == "clear" {
            if args.len() == 3 {
                Ok(Mode::Clear(PathBuf::from(&args[2])))
            } else {
                Err(())
            }
        } else {
            Ok(Mode::Cement(PathBuf::from(&args[1])))
        }
    } else {
        Err(())
    }
}
