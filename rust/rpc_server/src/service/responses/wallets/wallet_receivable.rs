use rsnano_core::{Account, Amount, BlockHash, PendingKey, Epoch};
use rsnano_node::node::Node;
use rsnano_rpc_messages::{ErrorDto, ReceivableDto, WalletReceivableArgs, SourceInfo};
use serde_json::json;
use std::{collections::HashMap, sync::Arc};

pub async fn wallet_receivable(node: Arc<Node>, enable_control: bool, args: WalletReceivableArgs) -> String {
    if !enable_control {
        return json!({"error": "RPC control is disabled"}).to_string();
    }

    let accounts = match node.wallets.get_accounts_of_wallet(&args.wallet) {
        Ok(accounts) => accounts,
        Err(e) => return json!({"error": e.to_string()}).to_string(),
    };

    let tx = node.ledger.read_txn();
    let mut block_source = HashMap::new();
    let mut block_threshold = HashMap::new();
    let mut block_default = HashMap::new();

    for account in accounts {
        let mut account_blocks_source: HashMap<BlockHash, SourceInfo> = HashMap::new();
        let mut account_blocks_threshold: HashMap<BlockHash, Amount> = HashMap::new();
        let mut account_blocks_default: Vec<BlockHash> = Vec::new();
        for (key, info) in node.ledger.any().receivable_upper_bound(&tx, account)
            .take(args.count as usize)
        {
            if args.include_only_confirmed.unwrap_or(true) && 
               !node.ledger.confirmed().block_exists_or_pruned(&tx, &key.send_block_hash) {
                continue;
            }

            if let Some(threshold) = args.threshold {
                if info.amount < threshold {
                    continue;
                }
            }

            if args.source.unwrap_or(false) || args.min_version.unwrap_or(false) {
                let source_info = SourceInfo {
                    amount: info.amount,
                    source: info.source,
                };
                account_blocks_source.insert(key.send_block_hash, source_info);
            } else if args.threshold.is_some() {
                account_blocks_threshold.insert(key.send_block_hash, info.amount);
            } else {
                account_blocks_default.push(key.send_block_hash);
            }
        }

        if !account_blocks_source.is_empty() {
            block_source.insert(account, account_blocks_source);
        }
        if !account_blocks_threshold.is_empty() {
            block_threshold.insert(account, account_blocks_threshold);
        }
        if !account_blocks_default.is_empty() {
            block_default.insert(account, account_blocks_default);
        }
    }

    let result = if args.source.unwrap_or(false) || args.min_version.unwrap_or(false) {
        ReceivableDto::Source { blocks: block_source }
    } else if args.threshold.is_some() {
        ReceivableDto::Threshold { blocks: block_threshold }
    } else {
        ReceivableDto::Blocks { blocks: block_default }
    };

    serde_json::to_string(&result).unwrap()
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_core::{Account, Amount, BlockEnum, PublicKey, RawKey, StateBlock, WalletId, DEV_GENESIS_KEY};
    use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
    use rsnano_node::{node::Node, wallets::WalletsExt};
    use rsnano_rpc_messages::ReceivableDto;
    use std::sync::Arc;
    use std::time::Duration;
    use test_helpers::{assert_timely_msg, System};

    fn send_block(node: Arc<Node>, account: Account) -> BlockEnum {
        let send1 = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            *DEV_GENESIS_HASH,
            *DEV_GENESIS_PUB_KEY,
            Amount::MAX - Amount::raw(1),
            account.into(),
            &DEV_GENESIS_KEY,
            node.work_generate_dev((*DEV_GENESIS_HASH).into()),
        ));

        node.process_active(send1.clone());
        assert_timely_msg(
            Duration::from_secs(5),
            || node.active.active(&send1),
            "not active on node 1",
        );

        send1
    }

    #[test]
    fn wallet_receivable_include_only_confirmed_false() {
        let mut system = System::new();
        let node = system.make_node();

        let wallet = WalletId::zero();
        node.wallets.create(wallet);
        let private_key = RawKey::zero();
        let public_key: PublicKey = (&private_key).try_into().unwrap();
        node.wallets.insert_adhoc2(&wallet, &private_key, false).unwrap();

        let send = send_block(node.clone(), public_key.into());

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let result = node.tokio.block_on(async {
            rpc_client
                .wallet_receivable(wallet, 1, None, None, None, Some(false))
                .await
                .unwrap()
        });
        
        println!("{:?}", result);

        if let ReceivableDto::Blocks { blocks } = result {
            assert_eq!(blocks.get(&public_key.into()).unwrap(), &vec![send.hash()]);
        } else {
            panic!("Expected ReceivableDto::Blocks");
        }

        server.abort();
    }

    #[test]
    fn wallet_receivable_options_none() {
        let mut system = System::new();
        let node = system.make_node();

        let wallet = WalletId::zero();
        node.wallets.create(wallet);
        let private_key = RawKey::zero();
        let public_key: PublicKey = (&private_key).try_into().unwrap();
        node.wallets.insert_adhoc2(&wallet, &private_key, false).unwrap();

        let _send = send_block(node.clone(), public_key.into());

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let result = node.tokio.block_on(async {
            rpc_client
                .wallet_receivable(wallet, 1, None, None, None, None)
                .await
                .unwrap()
        });

        server.abort();
    }

    #[test]
    fn wallet_receivable_threshold_some() {
        let mut system = System::new();
        let node = system.make_node();

        let wallet = WalletId::zero();
        node.wallets.create(wallet);
        let private_key = RawKey::zero();
        let public_key: PublicKey = (&private_key).try_into().unwrap();
        node.wallets.insert_adhoc2(&wallet, &private_key, false).unwrap();

        let _send = send_block(node.clone(), public_key.into());

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let result = node.tokio.block_on(async {
            rpc_client
                .wallet_receivable(
                    wallet,
                    1,
                    Some(Amount::raw(2)), 
                    None,
                    None,
                    Some(false),
                )
                .await
                .unwrap()
        });

        server.abort();
    }

    #[test]
    fn wallet_receivable_fails_without_enable_control() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let result = node
            .tokio
            .block_on(async { rpc_client.wallet_receivable(WalletId::zero(), 1, None, None, None, None).await });

        assert_eq!(
            result.err().map(|e| e.to_string()),
            Some("node returned error: \"RPC control is disabled\"".to_string())
        );

        server.abort();
    }
}

