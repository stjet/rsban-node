use rsnano_core::{Account, Amount, BlockHash};
use rsnano_node::Node;
use rsnano_rpc_messages::{ReceivableArgs, ReceivableDto, SourceInfo};
use serde_json::to_string_pretty;
use std::collections::HashMap;
use std::sync::Arc;

pub async fn receivable(node: Arc<Node>, args: ReceivableArgs) -> String {
    let transaction = node.store.tx_begin_read();
    let receivables = node.ledger.any().account_receivable_upper_bound(
        &transaction,
        args.account,
        BlockHash::zero(),
    );

    let mut blocks_source: HashMap<Account, HashMap<BlockHash, SourceInfo>> = HashMap::new();
    let mut blocks_threshold: HashMap<Account, HashMap<BlockHash, Amount>> = HashMap::new();
    let mut blocks_default: HashMap<Account, Vec<BlockHash>> = HashMap::new();

    let mut account_blocks_source: Vec<(BlockHash, SourceInfo)> = Vec::new();
    let mut account_blocks_threshold: Vec<(BlockHash, Amount)> = Vec::new();
    let mut account_blocks: Vec<BlockHash> = Vec::new();

    for (key, info) in receivables {
        if args.include_only_confirmed.unwrap_or(true)
            && !node
                .ledger
                .confirmed()
                .block_exists_or_pruned(&transaction, &key.send_block_hash)
        {
            continue;
        }

        if let Some(threshold) = args.threshold {
            if info.amount < threshold {
                continue;
            }
        }

        if args.source.unwrap_or(false) {
            account_blocks_source.push((
                key.send_block_hash,
                SourceInfo {
                    amount: info.amount,
                    source: info.source,
                },
            ));
        } else if args.threshold.is_some() {
            account_blocks_threshold.push((key.send_block_hash, info.amount));
        } else {
            account_blocks.push(key.send_block_hash);
        }

        if account_blocks.len() >= args.count as usize
            || account_blocks_threshold.len() >= args.count as usize
            || account_blocks_source.len() >= args.count as usize
        {
            break;
        }
    }

    if args.sorting.unwrap_or(false) {
        if args.source.unwrap_or(false) {
            account_blocks_source.sort_by(|a, b| b.1.amount.cmp(&a.1.amount));
        } else if args.threshold.is_some() {
            account_blocks_threshold.sort_by(|a, b| b.1.cmp(&a.1));
        }
        // Note: We don't sort account_blocks as it's only used for the simple case
    }

    // Apply offset and limit
    let offset = 0; //args.offset.unwrap_or(0) as usize;
    let count = args.count as usize;

    let receivable_dto = if args.source.unwrap_or(false) {
        blocks_source.insert(
            args.account,
            account_blocks_source
                .into_iter()
                .skip(offset)
                .take(count)
                .collect::<HashMap<_, _>>(),
        );
        ReceivableDto::Source {
            blocks: blocks_source,
        }
    } else if args.threshold.is_some() {
        blocks_threshold.insert(
            args.account,
            account_blocks_threshold
                .into_iter()
                .skip(offset)
                .take(count)
                .collect(),
        );
        ReceivableDto::Threshold {
            blocks: blocks_threshold,
        }
    } else {
        blocks_default.insert(
            args.account,
            account_blocks
                .into_iter()
                .skip(offset)
                .take(count)
                .collect(),
        );
        ReceivableDto::Blocks {
            blocks: blocks_default,
        }
    };

    to_string_pretty(&receivable_dto).unwrap()
}

#[cfg(test)]
mod tests {
    use rsnano_core::{
        Account, Amount, BlockEnum, PublicKey, RawKey, StateBlock, WalletId, DEV_GENESIS_KEY,
    };
    use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
    use rsnano_node::{wallets::WalletsExt, Node};
    use rsnano_rpc_messages::ReceivableDto;
    use std::sync::Arc;
    use std::time::Duration;
    use test_helpers::{assert_timely_msg, setup_rpc_client_and_server, System};

    fn send_block(node: Arc<Node>, account: Account, amount: Amount) -> BlockEnum {
        let transaction = node.ledger.read_txn();
        let previous = node
            .ledger
            .any()
            .account_head(&transaction, &*DEV_GENESIS_ACCOUNT)
            .unwrap_or(*DEV_GENESIS_HASH);
        let balance = node
            .ledger
            .any()
            .account_balance(&transaction, &*DEV_GENESIS_ACCOUNT)
            .unwrap_or(Amount::MAX);

        let send = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            previous,
            *DEV_GENESIS_PUB_KEY,
            balance - amount,
            account.into(),
            &DEV_GENESIS_KEY,
            node.work_generate_dev(previous.into()),
        ));

        node.process_active(send.clone());
        assert_timely_msg(
            Duration::from_secs(5),
            || node.active.active(&send),
            "not active on node",
        );

        send
    }

    #[test]
    fn receivable_include_only_confirmed() {
        let mut system = System::new();
        let node = system.make_node();

        let wallet = WalletId::zero();
        node.wallets.create(wallet);
        let private_key = RawKey::zero();
        let public_key: PublicKey = (&private_key).try_into().unwrap();
        node.wallets
            .insert_adhoc2(&wallet, &private_key, false)
            .unwrap();

        let send = send_block(node.clone(), public_key.into(), Amount::raw(1));

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let result1 = node.runtime.block_on(async {
            rpc_client
                .receivable(public_key.into(), 1, None, None, None, None, Some(true))
                .await
                .unwrap()
        });

        if let ReceivableDto::Blocks { blocks } = result1 {
            assert!(blocks.get(&public_key.into()).unwrap().is_empty());
        } else {
            panic!("Expected ReceivableDto::Blocks variant");
        }

        let result2 = node.runtime.block_on(async {
            rpc_client
                .receivable(public_key.into(), 1, None, None, None, None, Some(false))
                .await
                .unwrap()
        });

        if let ReceivableDto::Blocks { blocks } = result2 {
            assert_eq!(blocks.get(&public_key.into()).unwrap(), &vec![send.hash()]);
        } else {
            panic!("Expected ReceivableDto::Blocks variant");
        }

        server.abort();
    }

    #[test]
    fn receivable_options_none() {
        let mut system = System::new();
        let node = system.make_node();

        let wallet = WalletId::zero();
        node.wallets.create(wallet);
        let private_key = RawKey::zero();
        let public_key: PublicKey = (&private_key).try_into().unwrap();
        node.wallets
            .insert_adhoc2(&wallet, &private_key, false)
            .unwrap();

        let send = send_block(node.clone(), public_key.into(), Amount::raw(1));
        node.ledger.confirm(&mut node.ledger.rw_txn(), send.hash());

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let result = node.runtime.block_on(async {
            rpc_client
                .receivable(public_key.into(), 1, None, None, None, None, Some(true))
                .await
                .unwrap()
        });

        if let ReceivableDto::Blocks { blocks } = result {
            assert_eq!(blocks.get(&public_key.into()).unwrap(), &vec![send.hash()]);
        } else {
            panic!("Expected ReceivableDto::Blocks variant");
        }

        server.abort();
    }

    #[test]
    fn receivable_threshold_some() {
        let mut system = System::new();
        let node = system.make_node();

        let wallet = WalletId::zero();
        node.wallets.create(wallet);
        let private_key = RawKey::zero();
        let public_key: PublicKey = (&private_key).try_into().unwrap();
        node.wallets
            .insert_adhoc2(&wallet, &private_key, false)
            .unwrap();

        let send = send_block(node.clone(), public_key.into(), Amount::raw(1));
        node.ledger.confirm(&mut node.ledger.rw_txn(), send.hash());
        let send2 = send_block(node.clone(), public_key.into(), Amount::raw(2));
        node.ledger.confirm(&mut node.ledger.rw_txn(), send2.hash());

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let result = node.runtime.block_on(async {
            rpc_client
                .receivable(
                    public_key.into(),
                    2,
                    Some(Amount::raw(1)),
                    None,
                    None,
                    None,
                    None,
                )
                .await
                .unwrap()
        });

        println!("{:?}", result);

        if let ReceivableDto::Threshold { blocks } = result {
            assert_eq!(
                blocks
                    .get(&public_key.into())
                    .unwrap()
                    .get(&send2.hash())
                    .unwrap(),
                &Amount::raw(2)
            );
        } else {
            panic!("Expected ReceivableDto::Threshold variant");
        }

        server.abort();
    }

    #[test]
    fn receivable_sorting() {
        let mut system = System::new();
        let node = system.make_node();

        let wallet = WalletId::zero();
        node.wallets.create(wallet);
        let private_key = RawKey::zero();
        let public_key: PublicKey = (&private_key).try_into().unwrap();
        node.wallets
            .insert_adhoc2(&wallet, &private_key, false)
            .unwrap();

        let send = send_block(node.clone(), public_key.into(), Amount::raw(1));

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let result = node.runtime.block_on(async {
            rpc_client
                .receivable(
                    public_key.into(),
                    1,
                    None,
                    None,
                    None,
                    Some(true),
                    Some(false),
                )
                .await
                .unwrap()
        });

        if let ReceivableDto::Blocks { blocks } = result {
            assert_eq!(blocks.len(), 1);
            let recv_blocks = blocks.get(&public_key.into()).unwrap();
            assert_eq!(recv_blocks.len(), 1);
            assert_eq!(recv_blocks[0], send.hash());
        } else {
            panic!("Expected ReceivableDto::Blocks variant");
        }

        server.abort();
    }
}
