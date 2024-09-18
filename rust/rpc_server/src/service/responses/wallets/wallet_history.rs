use std::sync::Arc;
use rsnano_core::{Account, Amount, Block, BlockEnum, BlockHash, BlockSubType, PublicKey, WalletId};
use rsnano_node::node::Node;
use rsnano_rpc_messages::{HistoryEntryDto, WalletHistoryDto};
use rsnano_store_lmdb::Transaction;
use serde_json::to_string_pretty;

pub async fn wallet_history(node: Arc<Node>, wallet: WalletId, modified_since: Option<u64>) -> String {
    let accounts = node.wallets.get_accounts_of_wallet(&wallet).unwrap();
    let mut entries: Vec<HistoryEntryDto> = Vec::new();

    let block_transaction = node.store.tx_begin_read();

    for account in accounts {
        if let Some(info) = node.ledger.any().get_account(&block_transaction, &account) {
            let mut hash = info.head;

            while !hash.is_zero() {
                if let Some(block) = node.ledger.get_block(&block_transaction, &hash) {
                    let timestamp = block.sideband().map(|sideband| sideband.timestamp).unwrap_or_default();

                    if timestamp >= modified_since.unwrap_or(0) {
                        let entry = process_block(&node, &block_transaction, &block, &account, &hash, timestamp);
                        
                        if let Some(entry) = entry {
                            entries.push(entry);
                        }

                        hash = block.previous();
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
    }

    entries.sort_by(|a, b| b.local_timestamp.cmp(&a.local_timestamp));
    let wallet_history_dto = WalletHistoryDto::new(entries);

    to_string_pretty(&wallet_history_dto).unwrap()
}

fn process_block(node: &Arc<Node>, transaction: &dyn Transaction, block: &BlockEnum, block_account: &Account, hash: &BlockHash, timestamp: u64) -> Option<HistoryEntryDto> {
    match block {
        BlockEnum::State(state_block) => {
            let balance = state_block.balance();
            let previous_balance = node.ledger.any().block_balance(transaction, &state_block.previous()).unwrap_or(Amount::zero());

            if balance < previous_balance {
                // Send
                let account: Account = state_block.link().into();
                Some(HistoryEntryDto::new(
                    BlockSubType::Send,
                    account,
                    previous_balance - balance,
                    *block_account,
                    *hash,
                    timestamp,
                ))
            } else if !state_block.link().is_zero() && balance > previous_balance {
                // Receive
                let source_account = node.ledger.any().block_account(transaction, &state_block.link().into())
                    .unwrap_or_else(|| Account::from(state_block.link()));
                Some(HistoryEntryDto::new(
                    BlockSubType::Receive,
                    source_account,
                    balance - previous_balance,
                    *block_account,
                    *hash,
                    timestamp,
                ))
            } else {
                // Change or Epoch (ignored)
                None
            }
        },
        _ => None, // Ignore legacy blocks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_core::{Amount, BlockEnum, BlockHash, KeyPair, StateBlock, WalletId, DEV_GENESIS_KEY};
    use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
    use rsnano_node::wallets::WalletsExt;
    use test_helpers::System;
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;

    fn setup_test_environment(node: Arc<Node>, keys: KeyPair, send_amount: Amount) -> BlockHash {
        let send1 = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            *DEV_GENESIS_HASH,
            *DEV_GENESIS_PUB_KEY,
            Amount::MAX - send_amount,
            keys.account().into(),
            &DEV_GENESIS_KEY,
            node.work_generate_dev((*DEV_GENESIS_HASH).into()),
        ));

        node.process(send1.clone()).unwrap();

        let open_block = BlockEnum::State(StateBlock::new(
            keys.account(),
            BlockHash::zero(),
            keys.public_key(),
            send_amount,
            send1.hash().into(),
            &keys,
            node.work_generate_dev(keys.public_key().into()),
        ));

        node.process(open_block.clone()).unwrap();

        open_block.hash()
    }

    #[test]
    fn wallet_history() {
        let mut system = System::new();
        let node = system.build_node().finish();
        let keys = KeyPair::new();
        let send_amount = Amount::from(100);
        let open_hash = setup_test_environment(node.clone(), keys.clone(), send_amount);

        let wallet_id = WalletId::zero();
        node.wallets.create(wallet_id);
        node.wallets.insert_adhoc2(&wallet_id, &keys.private_key(), true).unwrap();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let result = node.tokio.block_on(async {
            rpc_client.wallet_history(wallet_id, None).await.unwrap()
        });

        server.abort();
    }
}