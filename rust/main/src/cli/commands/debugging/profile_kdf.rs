use anyhow::Result;
use clap::Parser;
use rsnano_core::{KeyDerivationFunction, Networks};
use rsnano_node::{config::NetworkConstants, NetworkParams};
use std::str::FromStr;
use std::time::Instant;

#[derive(Parser)]
pub(crate) struct ProfileKdfArgs {
    network: Option<String>,
}

impl ProfileKdfArgs {
    pub(crate) fn profile_kdf(&self) -> Result<()> {
        let network_params = if let Some(network) = &self.network {
            NetworkParams::new(Networks::from_str(&network).unwrap())
        } else {
            NetworkParams::new(NetworkConstants::active_network())
        };

        let kdf = KeyDerivationFunction::new(network_params.kdf_work);

        let begin = Instant::now();

        kdf.hash_password("", &[0; 32]);

        let end = Instant::now();
        let duration = end.duration_since(begin).as_micros();

        println!("Derivation time: {}", duration);

        Ok(())
    }
}
