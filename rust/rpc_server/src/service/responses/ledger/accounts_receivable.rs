use std::sync::Arc;
use rsnano_core::{Account, Amount, BlockHash};
use rsnano_node::node::Node;
use rsnano_rpc_messages::{AccountsReceivableArgs, AccountsReceivablesDto};
use serde_json::to_string_pretty;
use std::collections::BTreeMap;

pub async fn accounts_receivable(node: Arc<Node>, args: AccountsReceivableArgs) -> String {
    let mut blocks: BTreeMap<Account, Vec<BlockHash>> = BTreeMap::new();
    let transaction = node.store.tx_begin_read();

    let simple = args.threshold.unwrap_or(Amount::zero()).is_zero() && !args.source.unwrap_or(false) && !args.sorting.unwrap_or(false);

    for account in args.accounts {
        let mut receivable_hashes = Vec::new();

        let mut iter = node.ledger.any().account_receivable_upper_bound(&transaction, account, BlockHash::zero());
        let mut count = 0;

        while let Some((key, info)) = iter.next() {
            if count >= args.count as usize {
                break;
            }

            if args.include_only_confirmed.unwrap_or(true) && !node.ledger.confirmed().block_exists_or_pruned(&transaction, &key.send_block_hash) {
                continue;
            }

            if info.amount < args.threshold.unwrap_or(Amount::zero()) {
                continue;
            }

            receivable_hashes.push(key.send_block_hash);
            count += 1;
        }

        if !receivable_hashes.is_empty() {
            blocks.insert(account, receivable_hashes);
        }
    }

    if args.sorting.unwrap_or(false) && !simple {
        for (_, receivables) in blocks.iter_mut() {
            receivables.sort_by(|a, b| {
                let amount_a = node.ledger.any().get_block(&transaction, a).unwrap().balance();
                let amount_b = node.ledger.any().get_block(&transaction, b).unwrap().balance();
                amount_b.cmp(&amount_a)
            });
        }
    }

    to_string_pretty(&AccountsReceivablesDto::new("blocks".to_string(), blocks)).unwrap()
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_core::{Amount, BlockEnum, StateBlock, DEV_GENESIS_KEY};
    use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
    use rsnano_node::node::Node;
    use std::sync::Arc;
    use std::time::Duration;
    use test_helpers::{assert_timely_msg, System};

    fn send_block(node: Arc<Node>) -> BlockEnum {
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

        send1
    }

    #[test]
    fn accounts_receivable() {
        let mut system = System::new();
        let node = system.make_node();

        let send = send_block(node.clone());

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let result = node.tokio.block_on(async {
            rpc_client
                .accounts_receivable(vec![DEV_GENESIS_KEY.public_key().as_account()], 1, None, None, None, None, Some(false))
                .await
                .unwrap()
        });

        assert_eq!(result.value.get(&DEV_GENESIS_KEY.public_key().as_account()).unwrap(), &vec![send.hash()]);

        server.abort();
    }
}