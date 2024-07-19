use anyhow::Result;
use clap::Parser;
use rsnano_core::{BlockHash, KeyDerivationFunction, Networks};
use rsnano_node::{config::NetworkConstants, NetworkParams};
use std::str::FromStr;
use std::time::Instant;

#[derive(Parser)]
pub(crate) struct ProfileValidateArgs {
    network: Option<String>,
}

impl ProfileValidateArgs {
    pub(crate) fn profile_validate(&self) -> Result<()> {
        let network_params = if let Some(network) = &self.network {
            NetworkParams::new(Networks::from_str(&network).unwrap())
        } else {
            NetworkParams::new(NetworkConstants::active_network())
        };

        println!("Starting validation profile");

        let start = Instant::now();
        let mut valid = false;
        let hash = BlockHash::default();
        let count: u64 = 10_000_000;

        for i in 0..count {
            //valid = network_params.work.value(&hash, i) > network_params.work.difficulty;
        }

        let total_time = start.elapsed().as_nanos();
        let average = total_time / count as u128;

        println!(
            "Average validation time: {} ns ({} validations/s)",
            average,
            (count as u128 * 1_000_000_000) / total_time
        );

        Ok(())
    }
}
