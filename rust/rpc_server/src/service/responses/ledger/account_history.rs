use std::sync::Arc;
use rsnano_core::{Account, Amount, BlockType, PublicKey};
use rsnano_node::node::Node;
use rsnano_rpc_messages::{AccountHistoryArgs, AccountHistoryDto, ErrorDto, HistoryEntry};
use serde_json::to_string_pretty;

pub async fn account_history(node: Arc<Node>, args: AccountHistoryArgs) -> String {
    // Extract arguments
    let account = args.account;
    let count = args.count;
    let offset = args.offset.unwrap_or(0);
    let reverse = args.reverse.unwrap_or(false);
    let raw = args.raw.unwrap_or(false);
    let head = args.head;

    let transaction = node.store.tx_begin_read();

    // Determine starting hash
    let mut hash = if let Some(head) = head {
        if node.ledger.get_block(&transaction, &head).is_some() {
            Some(head)
        } else {
            return to_string_pretty(&AccountHistoryDto {
                account: account,
                history: Vec::new(),
                previous: None,
                next: None,
            }).unwrap();
        }
    } else if reverse {
        node.ledger.account_info(&transaction, &account)
            .map(|info| info.open_block)
    } else {
        Some(node.ledger.account_info(&transaction, &account).unwrap().head)
    };

    if hash.is_none() {
        return to_string_pretty(&AccountHistoryDto {
            account: account,
            history: Vec::new(),
            previous: None,
            next: None,
        }).unwrap_or_else(|_| "{}".to_string());
    }

    let mut history = Vec::new();
    let mut remaining_count = count;
    let mut current_offset = offset;

    while let Some(current_hash) = hash {
        if remaining_count == 0 {
            break;
        }

        if let Some(block) = node.ledger.get_block(&transaction, &current_hash) {
            if current_offset > 0 {
                current_offset -= 1;
            } else {
                let mut entry = HistoryEntry {
                    hash: current_hash,
                    local_timestamp: block.sideband().unwrap().timestamp,
                    height: block.sideband().unwrap().height,
                    confirmed: node.ledger.confirmed().block_exists_or_pruned(&transaction, &current_hash),
                    work: None,
                    signature: None,
                    block_type: match block.block_type().try_into() {
                        Ok(bt) => bt,
                        Err(_) => return to_string_pretty(&ErrorDto::new("Invalid block type".to_string())).unwrap(),
                    },
                    account: block.account(),
                    amount: block.balance(),
                };

                // Add raw block data if requested
                if raw {
                    entry.work = Some(block.work().into());
                    entry.signature = Some(block.block_signature().clone());
                }

                let tx = node.ledger.read_txn();

                // Implement history_visitor logic here
                match block.block_type() {
                    BlockType::LegacySend => {
                        //entry.account = block.link_field().into();
                        entry.amount = node.ledger.any().block_balance(&tx, &block.previous()).unwrap() - block.balance();
                    },
                    BlockType::LegacyReceive | BlockType::LegacyOpen => {
                        //entry.account = block.source().unwrap_or_default();
                        //entry.amount = block.balance().saturating_sub(block.previous_balance());
                    },
                    BlockType::LegacyChange => {
                        entry.account = block.representative_field().unwrap().into();
                        //entry.amount = Amount::zero();
                    },
                    BlockType::State => {
                        // Handle state blocks based on subtype
                        if block.is_send() {
                            //entry.account = block.link_field().into();
                            entry.amount = node.ledger.any().block_balance(&tx, &block.previous()).unwrap() - block.balance();
                        } else if block.is_receive() {
                            //entry.account = block.link_field().into();
                            //entry.amount = block.balance().saturating_sub(block.previous_balance());
                        } else if block.is_open() {
                            //entry.account = block.source().unwrap_or_default();
                            //entry.amount = block.balance();
                        } else if block.is_change() {
                            entry.account = block.representative_field().unwrap().into();
                            //entry.amount = Amount::zero();
                        } else if block.is_epoch() {
                            entry.account = Account::zero();
                            //entry.amount = Amount::zero();
                        }
                    },
                    _ => return to_string_pretty(&ErrorDto::new("error".to_string())).unwrap()
                }

                // Implement filtering logic
                if !args.account_filter.clone().unwrap_or_default().is_empty() {
                    if args.account_filter.clone().unwrap().contains(&entry.account.into()) {
                        history.push(entry);
                        remaining_count -= 1;
                    }
                } else {
                    history.push(entry);
                    remaining_count -= 1;
                }
            }
        }

        hash = if reverse {
            node.ledger.any().block_successor(&transaction, &current_hash)
        } else {
            node.ledger.get_block(&transaction, &current_hash)
                .and_then(|block| Some(block.previous()))
        };
    }

    to_string_pretty(&AccountHistoryDto {
        account: account,
        history,
        previous: if reverse { None } else { hash },
        next: if reverse { hash } else { None },
    }).unwrap()
}


#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH};
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

        //let change = node.wallets.change_action2(wallet_id, DEV_GENESIS_ACCOUNT.public_key(), DEV_GENESIS_ACCOUNT.public_key()).await.unwrap();
        let send = node.wallets.send_action2(&wallet_id, *DEV_GENESIS_ACCOUNT, *DEV_GENESIS_ACCOUNT, node.config.receive_minimum, node.work_generate_dev((*DEV_GENESIS_HASH).into()), false, None).unwrap();
        //let receive = node.wallets.receive_action2(wallet_id, send.hash(), DEV_GENESIS_ACCOUNT.public_key(), node.config.receive_minimum, send.destination()).await.unwrap();

        //let usend = node.wallets.send_action2(wallet_id, DEV_GENESIS_ACCOUNT.public_key(), DEV_GENESIS_ACCOUNT.public_key(), Amount::from_raw(1_000_000)).await.unwrap();
        //let ureceive = node.wallets.receive_action2(wallet_id, usend.hash(), DEV_GENESIS_ACCOUNT.public_key(), Amount::from_raw(1_000_000), usend.destination()).await.unwrap();
        //let uchange = node.wallets.change_action2(wallet_id, DEV_GENESIS_ACCOUNT.public_key(), PublicKey::random()).await.unwrap();

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
        assert_eq!(account_history.history.len(), 2);

        // Verify history entries
        let history = account_history.history;
        assert_eq!(history[0].block_type, BlockSubType::Send);
        assert_eq!(history[0].hash, send.hash());
        assert_eq!(history[0].account, *DEV_GENESIS_ACCOUNT);
        assert_eq!(history[0].amount, node.config.receive_minimum);
        assert_eq!(history[0].height, 2);
        assert!(!history[0].confirmed);

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
        assert!(history[4].confirmed);

        // Test count and reverse
        let result = rpc_client.account_history(
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