use std::{sync::Arc, time::Duration};
use rsnano_core::{Account, BlockEnum, BlockHash, WalletId};
use rsnano_node::node::{Node, NodeExt};
use rsnano_rpc_messages::{BlocksDto, ErrorDto};
use serde_json::to_string_pretty;
use std::collections::VecDeque;
use anyhow::{Result, Context};

pub async fn wallet_republish(node: Arc<Node>, enable_control: bool, wallet: WalletId, count: u64) -> String {
    if !enable_control {
        return to_string_pretty(&ErrorDto::new("RPC control is disabled".to_string())).unwrap_or_else(|_| "{{\"error\":\"RPC control is disabled\"}}".to_string());
    }

    let accounts = match node.wallets.get_accounts_of_wallet(&wallet) {
        Ok(accounts) => accounts,
        Err(_) => return to_string_pretty(&ErrorDto::new("Failed to get accounts of wallet".to_string())).unwrap_or_else(|_| "{{\"error\":\"Failed to get accounts of wallet\"}}".to_string()),
    };
    
    match collect_blocks_to_republish(node.clone(), accounts, count).await {
        Ok((blocks, republish_bundle)) => {
            node.flood_block_many(republish_bundle.into(), Box::new(|| ()), Duration::from_millis(25));
            to_string_pretty(&BlocksDto::new(blocks)).unwrap_or_else(|_| "{{\"blocks\":[]}}".to_string())
        },
        Err(_) => to_string_pretty(&ErrorDto::new("Failed to collect blocks to republish".to_string())).unwrap_or_else(|_| "{{\"error\":\"Failed to collect blocks to republish\"}}".to_string()),
    }
}

async fn collect_blocks_to_republish(node: Arc<Node>, accounts: Vec<Account>, count: u64) -> Result<(Vec<BlockHash>, VecDeque<BlockEnum>)> {
    let mut blocks = Vec::new();
    let mut republish_bundle = VecDeque::new();
    let tx = node.ledger.read_txn();

    for account in accounts {
        let mut latest = node.ledger.any().account_head(&tx, &account)
            .context("Failed to get account head")?;
        let mut hashes = Vec::new();

        while !latest.is_zero() && hashes.len() < count as usize {
            hashes.push(latest);
            if let Some(block) = node.ledger.get_block(&tx, &latest) {
                latest = block.previous();
            } else {
                latest = BlockHash::zero();
            }
        }

        for hash in hashes.into_iter().rev() {
            if let Some(block) = node.ledger.get_block(&tx, &hash) {
                republish_bundle.push_back(block);
                blocks.push(hash);
            }
        }
    }

    Ok((blocks, republish_bundle))
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_core::{Amount, BlockEnum, RawKey, StateBlock, WalletId, DEV_GENESIS_KEY};
    use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
    use rsnano_node::{node::Node, wallets::WalletsExt};
    use test_helpers::{assert_timely_msg, System};

    fn send_block(node: Arc<Node>) {
        let send1 = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            *DEV_GENESIS_HASH,
            *DEV_GENESIS_PUB_KEY,
            Amount::MAX - Amount::raw(1),
            DEV_GENESIS_KEY.account().into(),
            &DEV_GENESIS_KEY,
            node.work_generate_dev((*DEV_GENESIS_HASH).into()),
        ));

        node.process_active(send1.clone());
        assert_timely_msg(
            Duration::from_secs(5),
            || node.active.active(&send1),
            "not active on node 1",
        );
    }

    #[test]
    fn wallet_republish() {
        let mut system = System::new();
        let node = system.make_node();

        send_block(node.clone());

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let wallet = WalletId::zero();

        node.wallets.create(wallet);

        node.wallets
            .insert_adhoc2(&wallet, &DEV_GENESIS_KEY.private_key(), false)
            .unwrap();

        let result = node
            .tokio
            .block_on(async { rpc_client.wallet_republish(wallet, 1).await.unwrap() });

        server.abort();
    }

    #[test]
    fn wallet_republish_fails_without_enable_control() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let result = node
            .tokio
            .block_on(async { rpc_client.wallet_republish(WalletId::zero(), 1).await });

        assert_eq!(
            result.err().map(|e| e.to_string()),
            Some("node returned error: \"RPC control is disabled\"".to_string())
        );

        server.abort();
    }
}