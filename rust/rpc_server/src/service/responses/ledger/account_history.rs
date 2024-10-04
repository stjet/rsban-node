use std::sync::Arc;
use rsnano_core::{Account, Amount, Block, BlockEnum, BlockHash, BlockSubType, PublicKey};
use rsnano_ledger::DEV_GENESIS_HASH;
use rsnano_node::node::Node;
use rsnano_rpc_messages::{AccountHistoryArgs, AccountHistoryDto, HistoryEntry};
use serde_json::to_string_pretty;

pub async fn account_history(node: Arc<Node>, args: AccountHistoryArgs) -> String {
    let transaction = node.store.tx_begin_read();
    let mut history = Vec::new();
    let mut hash = args.head.unwrap_or_else(|| node.ledger.any().account_head(&transaction, &args.account).unwrap_or_default());
    let mut count = args.count;
    let mut offset = args.offset.unwrap_or(0);
    let reverse = args.reverse.unwrap_or(false);
    let raw = args.raw.unwrap_or(false);

    while let Some(block) = node.ledger.get_block(&transaction, &hash) {
        if offset > 0 {
            offset -= 1;
        } else if count > 0 {
            if raw {
                let entry = create_history_entry(node.clone(), &block, &hash, raw);
                if should_include_entry(&entry, &args.account_filter) {
                    history.push(entry);
                    count -= 1;
                }
            } 
            else {
                if block.is_receive() || block.is_send() {
                    let entry = create_history_entry(node.clone(), &block, &hash, raw);
                    if should_include_entry(&entry, &args.account_filter) {
                        history.push(entry);
                        count -= 1;
                    }
                }
            }
        } else {
            break;
        }

        hash = if reverse {
            node.ledger.any().block_successor(&transaction, &hash).unwrap_or_default()
        } else {
            block.previous()
        };

        if hash.is_zero() {
            break;
        }
    }

    if reverse {
        history.reverse();
    }

    let next = if !hash.is_zero() {
        Some(hash)
    } else {
        None
    };

    let previous = if !history.is_empty() {
        Some(if reverse {
            history.last().unwrap().hash
        } else {
            history.first().unwrap().hash
        })
    } else {
        None
    };

    let account_history = AccountHistoryDto {
        account: args.account,
        history,
        previous,
        next,
    };

    to_string_pretty(&account_history).unwrap_or_else(|_| "".to_string())
}

fn create_history_entry(node: Arc<Node>, block: &BlockEnum, hash: &BlockHash, raw: bool) -> HistoryEntry {
    let transaction = node.ledger.read_txn();
    let confirmed = node.ledger.confirmed().block_exists_or_pruned(&transaction, hash);
    let local_timestamp = block.sideband().unwrap().timestamp;
    let height = block.sideband().unwrap().height;

    let (block_type, account, amount) = match block {
        BlockEnum::LegacySend(send_block) => {
            let amount = node.ledger.any().block_amount(&transaction, hash).unwrap_or_default();
            (BlockSubType::Send, *send_block.destination(), amount)
        },
        BlockEnum::LegacyReceive(receive_block) => {
            let amount = node.ledger.any().block_amount(&transaction, hash).unwrap_or_default();
            let source_account = node.ledger.any().block_account(&transaction, &receive_block.source()).unwrap_or_default();
            (BlockSubType::Receive, source_account, amount)
        },
        BlockEnum::LegacyOpen(open_block) => {
            let amount = if open_block.source() == *DEV_GENESIS_HASH {
                node.ledger.constants.genesis_amount
            } else {
                node.ledger.any().block_amount(&transaction, hash).unwrap_or_default()
            };
            let source_account = node.ledger.any().block_account(&transaction, &open_block.source()).unwrap_or_default();
            (BlockSubType::Receive, source_account, amount)
        },
        BlockEnum::LegacyChange(change_block) => {
            (BlockSubType::Change, change_block.representative_field().unwrap().into(), Amount::zero())
        },
        BlockEnum::State(state_block) => {
            let (block_type, account, amount) = if state_block.previous() != BlockHash::zero() {
                let previous = state_block.previous();
                let previous_balance = node.ledger.any().block_balance(&transaction, &previous).unwrap_or_default();
                if state_block.balance() < previous_balance {
                    (BlockSubType::Send, Account::decode_hex(state_block.link().encode_hex()).unwrap(), previous_balance - state_block.balance())
                } else if state_block.link().is_zero() {
                    (BlockSubType::Change, state_block.representative_field().unwrap().into(), Amount::zero())
                } else {
                    let source_account = node.ledger.any().block_account(&transaction, &state_block.link().into()).unwrap_or_default();
                    (BlockSubType::Receive, source_account, state_block.balance() - previous_balance)
                }
            } else {
                (BlockSubType::Open, state_block.account(), state_block.balance())
            };
            (block_type, account, amount)
        },
    };

    HistoryEntry {
        block_type,
        account,
        amount,
        local_timestamp,
        height,
        hash: *hash,
        confirmed,
        work: if raw { Some(block.work().into()) } else { None },
        signature: if raw { Some(block.block_signature().clone()) } else { None },
    }
}

fn should_include_entry(entry: &HistoryEntry, account_filter: &Option<Vec<PublicKey>>) -> bool {
    account_filter
        .as_ref()
        .map(|filter| filter.contains(&entry.account.into()))
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
    use rsnano_node::wallets::WalletsExt;
    use test_helpers::System;
    use rsnano_core::{Account, Amount, BlockHash, BlockSubType, Epoch, PublicKey, Root, WalletId, DEV_GENESIS_KEY};
    use rsnano_rpc_messages::{AccountHistoryArgs, AccountHistoryDto, HistoryEntry};

    #[test]
    fn account_history() {
        let mut system = System::new();
        let node = system.make_node();

        // Create and process blocks
        let wallet_id = WalletId::zero();
        node.wallets.create(wallet_id);
        node.wallets.insert_adhoc2(&wallet_id, &DEV_GENESIS_KEY.private_key(), false).unwrap();

        let change = node.wallets.change_action2(
            &wallet_id, 
            *DEV_GENESIS_ACCOUNT, 
            *DEV_GENESIS_PUB_KEY, 
            node.work_generate_dev((*DEV_GENESIS_HASH).into()), 
            false)
        .unwrap().unwrap();

        let send = node.wallets.send_action2(
            &wallet_id, 
            *DEV_GENESIS_ACCOUNT, 
            *DEV_GENESIS_ACCOUNT, 
            node.config.receive_minimum, 
            node.work_generate_dev((*DEV_GENESIS_HASH).into()), 
            false, None)
        .unwrap();

        let receive = node.wallets.receive_action2(
            &wallet_id,
            send.hash(),
            *DEV_GENESIS_PUB_KEY,
            node.config.receive_minimum,
            *DEV_GENESIS_ACCOUNT,
            node.work_generate_dev(send.hash().into()),
            false
        ).unwrap().unwrap();

        /*let usend = node.wallets.send_action2(
            &wallet_id,
            *DEV_GENESIS_ACCOUNT,
            *DEV_GENESIS_ACCOUNT,
            Amount::raw(1_000_000),
            node.work_generate_dev((*DEV_GENESIS_HASH).into()),
            false,
            None
        ).unwrap();

        let ureceive = node.wallets.receive_action2(
            &wallet_id,
            usend.hash(),
            *DEV_GENESIS_PUB_KEY,
            Amount::raw(1_000_000),
            *DEV_GENESIS_ACCOUNT,
            node.work_generate_dev(usend.hash().into()),
            false
        ).unwrap().unwrap();

        let uchange = node.wallets.change_action2(
            &wallet_id,
            *DEV_GENESIS_ACCOUNT,
            PublicKey::zero(),
            node.work_generate_dev((*DEV_GENESIS_HASH).into()),
            false
        ).unwrap().unwrap();*/

        // Set up RPC client and server
        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let account_history = node.tokio.block_on(async {
            rpc_client.account_history(
                *DEV_GENESIS_ACCOUNT,
                100,
                Some(false),
                None,
                None,
                None,
                None,
            ).await.unwrap()
        });

        assert_eq!(account_history.account, *DEV_GENESIS_ACCOUNT);
        assert_eq!(account_history.history.len(), 3);

        // Verify history entries
        let history = account_history.history;
        assert_eq!(history[0].block_type, BlockSubType::Receive);
        assert_eq!(history[0].hash, receive.hash());
        assert_eq!(history[0].account, *DEV_GENESIS_ACCOUNT);
        assert_eq!(history[0].amount, node.config.receive_minimum);
        assert_eq!(history[0].height, 4);
        assert!(!history[0].confirmed);

        assert_eq!(history[1].block_type, BlockSubType::Send);
        assert_eq!(history[1].hash, send.hash());
        assert_eq!(history[1].account, *DEV_GENESIS_ACCOUNT);
        assert_eq!(history[1].amount, node.config.receive_minimum);
        assert_eq!(history[1].height, 3);
        assert!(!history[1].confirmed);

        assert_eq!(history[2].block_type, BlockSubType::Receive);
        assert_eq!(history[2].hash, *DEV_GENESIS_HASH);
        assert_eq!(history[2].account, *DEV_GENESIS_ACCOUNT);
        assert_eq!(history[2].amount, node.ledger.constants.genesis_amount);
        assert_eq!(history[2].height, 1);
        assert!(history[2].confirmed);
        //assert_eq!(history[0].block_type, BlockSubType::Send);
        //assert_eq!(history[0].hash, send.hash());
        //assert_eq!(history[0].account, *DEV_GENESIS_ACCOUNT);
        //assert_eq!(history[0].amount, node.config.receive_minimum);
        //assert_eq!(history[0].height, 2);
        //assert!(!history[0].confirmed);

        /*assert_eq!(history[1].block_type, BlockSubType::Send);
        assert_eq!(history[1].hash, usend.hash());
        assert_eq!(history[1].account, *DEV_GENESIS_ACCOUNT);
        assert_eq!(history[1].amount, Amount::from_raw(1_000_000));
        assert_eq!(history[1].height, 5);
        assert!(!history[1].confirmed);

        assert_eq!(history[2].block_type, BlockSubType::Receive);
        assert_eq!(history[2].hash, receive.hash());
        assert_eq!(history[2].account, *DEV_GENESIS_ACCOUNT);
        assert_eq!(history[2].amount, node.config.receive_minimum);
        assert_eq!(history[2].height, 4);
        assert!(!history[2].confirmed);

        assert_eq!(history[3].block_type, BlockSubType::Send);
        assert_eq!(history[3].hash, send.hash());
        assert_eq!(history[3].account, *DEV_GENESIS_ACCOUNT);
        assert_eq!(history[3].amount, node.config.receive_minimum);
        assert_eq!(history[3].height, 3);
        assert!(!history[3].confirmed);

        assert_eq!(history[4].block_type, BlockSubType::Receive);
        assert_eq!(history[4].hash, *DEV_GENESIS_HASH);
        assert_eq!(history[4].account, *DEV_GENESIS_ACCOUNT);
        assert_eq!(history[4].amount, Amount::max());
        assert_eq!(history[4].height, 1);
        assert!(history[4].confirmed);*/

        // Test count and reverse
        /*let result = rpc_client.account_history(
            AccountHistoryArgs::new(
                *DEV_GENESIS_ACCOUNT,
                1,
                Some(false),
                None,
                None,
                Some(true),
                None,
            )
        ).await.unwrap();

        let account_history: AccountHistoryDto = serde_json::from_str(&result).unwrap();
        assert_eq!(account_history.history.len(), 1);
        assert_eq!(account_history.history[0].height, 1);
        assert_eq!(account_history.next, Some(change.hash()));

        // Test filtering
        let account2 = PublicKey::random();
        node.wallets.deterministic_insert(wallet_id, true, account2).await.unwrap();
        let send2 = node.wallets.send_action(wallet_id, DEV_GENESIS_ACCOUNT.public_key(), account2, node.config.receive_minimum).await.unwrap();
        let receive2 = node.wallets.receive_action(wallet_id, send2.hash(), account2, node.config.receive_minimum, send2.destination()).await.unwrap();

        // Test filter for send state blocks
        let result = rpc_client.account_history(
            AccountHistoryArgs::new(
                *DEV_GENESIS_ACCOUNT,
                100,
                Some(false),
                None,
                None,
                None,
                Some(vec![account2]),
            )
        ).await.unwrap();

        let account_history: AccountHistoryDto = serde_json::from_str(&result).unwrap();
        assert_eq!(account_history.history.len(), 2);

        // Test filter for receive state blocks
        let result = rpc_client.account_history(
            AccountHistoryArgs::new(
                account2,
                100,
                Some(false),
                None,
                None,
                None,
                Some(vec![*DEV_GENESIS_ACCOUNT]),
            )
        ).await.unwrap();

        let account_history: AccountHistoryDto = serde_json::from_str(&result).unwrap();
        assert_eq!(account_history.history.len(), 1);*/

        server.abort();
    }
}