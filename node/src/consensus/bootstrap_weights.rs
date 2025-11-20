use rsnano_core::{
    utils::{BufferReader, Deserialize, StreamExt},
    Account, Amount, Networks, PublicKey,
};
use rsnano_ledger::{BootstrapWeights, RepWeightCache, RepWeights};
use tracing::info;

pub(crate) fn get_bootstrap_weights(network: Networks) -> BootstrapWeights {
    let buffer = get_bootstrap_weights_bin(network);
    deserialize_bootstrap_weights(buffer)
}

fn get_bootstrap_weights_bin(network: Networks) -> &'static [u8] {
    if network == Networks::NanoLiveNetwork {
        include_bytes!("../../rep_weights_live.bin")
    } else {
        include_bytes!("../../rep_weights_live.bin")
    }
}

fn deserialize_bootstrap_weights(buffer: &[u8]) -> BootstrapWeights {
    let mut reader = BufferReader::new(buffer);
    let mut weights = RepWeights::new();
    let mut max_blocks = 0;
    if let Ok(count) = reader.read_u128_be() {
        max_blocks = count as u64;
        loop {
            let Ok(account) = PublicKey::deserialize(&mut reader) else {
                break;
            };
            let Ok(weight) = Amount::deserialize(&mut reader) else {
                break;
            };
            weights.insert(account.into(), weight);
        }
    }

    BootstrapWeights {
        max_blocks,
        weights,
    }
}

/*
 * fn get_bootstrap_weights_text(network: Networks) -> &'static str {
    if network == Networks::NanoLiveNetwork {
        include_str!("../../rep_weights_live.txt")
    } else {
        include_str!("../../rep_weights_beta.txt")
    }
}

fn deserialize_bootstrap_weights(buffer: &str) -> BootstrapWeights {
    let mut weights = RepWeights::new();
    let mut first_line = true;
    let mut max_blocks = 0;
    for line in buffer.lines() {
        if first_line {
            max_blocks = line.parse().unwrap();
            first_line = false;
            continue;
        }

        let mut it = line.split(':');
        let account = Account::decode_account(it.next().unwrap()).unwrap();
        let weight = Amount::decode_dec(it.next().unwrap()).unwrap();
        weights.insert(account.into(), weight);
    }

    BootstrapWeights {
        max_blocks,
        weights,
    }
}
*/

pub(crate) fn log_bootstrap_weights(weight_cache: &RepWeightCache) {
    let mut bootstrap_weights = weight_cache.bootstrap_weights();
    if !bootstrap_weights.is_empty() {
        info!(
            "Initial bootstrap height: {}",
            weight_cache.bootstrap_weight_max_blocks()
        );
        info!("Current ledger height:    {}", weight_cache.block_count());

        // Use bootstrap weights if initial bootstrap is not completed
        if weight_cache.use_bootstrap_weights() {
            info!("Using predefined representative weights, since block count is less than bootstrap threshold");
            info!("************************************ Bootstrap weights ************************************");
            // Sort the weights
            let mut sorted_weights = bootstrap_weights.drain().collect::<Vec<_>>();
            sorted_weights.sort_by(|(_, weight_a), (_, weight_b)| weight_b.cmp(weight_a));

            for (rep, weight) in sorted_weights {
                info!(
                    "Using bootstrap rep weight: {} -> {}",
                    Account::from(&rep).encode_account(),
                    weight.format_balance(0)
                );
            }
            info!("************************************ ================= ************************************");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_weights_text() {
        assert_eq!(
            get_bootstrap_weights_text(Networks::NanoLiveNetwork).len(),
            14126,
            "expected live weights don't match'"
        );
        assert_eq!(
            get_bootstrap_weights_text(Networks::NanoBetaNetwork).len(),
            1161,
            "expected beta weights don't match'"
        );
    }

    #[test]
    fn bootstrap_weights() {
        let result = get_bootstrap_weights(Networks::NanoLiveNetwork);
        assert_eq!(result.weights.len(), 137);
        assert_eq!(result.max_blocks, 207_494_994);
    }
}
