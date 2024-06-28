use crate::cli::get_path;
use clap::Parser;
use rsnano_core::{Account, Amount, BlockHash, PublicKey, RawKey, SendBlock};
use rsnano_node::wallets::Wallets;
use std::{sync::Arc, time::Instant};

#[derive(Parser)]
pub(crate) struct DiagnosticsOptions {
    #[arg(long)]
    data_path: Option<String>,
    #[arg(long)]
    network: Option<String>,
}

impl DiagnosticsOptions {
    pub(crate) fn run(&self) {
        let path = get_path(&self.data_path, &self.network);

        let wallets = Arc::new(Wallets::new_null(&path).unwrap());

        println!("Testing hash function");

        SendBlock::new(
            &BlockHash::zero(),
            &Account::zero(),
            &Amount::zero(),
            &RawKey::zero(),
            &PublicKey::zero(),
            0,
        );

        println!("Testing key derivation function");

        wallets.kdf.hash_password("", &mut [0; 32]);

        println!("Testing time retrieval latency...");

        let iters = 2_000_000;
        let start = Instant::now();
        for _ in 0..iters {
            let _ = Instant::now();
        }
        let duration = start.elapsed();
        let avg_duration = duration.as_nanos() as f64 / iters as f64;

        println!("{} nanoseconds", avg_duration);
    }
}
