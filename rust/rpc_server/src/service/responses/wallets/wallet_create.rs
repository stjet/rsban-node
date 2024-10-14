use rsnano_core::{RawKey, WalletId};
use rsnano_node::{wallets::WalletsExt, Node};
use rsnano_rpc_messages::{ErrorDto, WalletCreateDto};
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn wallet_create(node: Arc<Node>, enable_control: bool, seed: Option<RawKey>) -> String {
    if !enable_control {
        return to_string_pretty(&ErrorDto::new("RPC control is disabled".to_string())).unwrap();
    }

    let wallet = WalletId::random();
    node.wallets.create(wallet);
    let wallet_create_dto = WalletCreateDto::new(wallet);

    if let Some(seed) = seed {
        node
            .wallets
            .change_seed(wallet, &seed, 0)
            .expect("This should not fail since the wallet was just created");
    }

    to_string_pretty(&wallet_create_dto).unwrap()
}
