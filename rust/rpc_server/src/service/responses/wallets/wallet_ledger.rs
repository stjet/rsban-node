use std::sync::Arc;
use rsnano_core::Account;
use rsnano_node::node::Node;
use rsnano_rpc_messages::{AccountInfo, ErrorDto, WalletLedgerArgs, WalletLedgerDto};
use serde_json::to_string_pretty;
use std::collections::HashMap;

pub async fn wallet_ledger(node: Arc<Node>, enable_control: bool, args: WalletLedgerArgs) -> String {
    if enable_control {
        let representative = args.representative.unwrap_or(false);
        let weight = args.weight.unwrap_or(false);
        let receivable = args.receivable.unwrap_or(args.receivable.unwrap_or(false));
        let modified_since = args.modified_since.unwrap_or(0);

        let wallet_id = args.wallet;
        
        let mut accounts_json: HashMap<Account, AccountInfo> = HashMap::new();

        if let Ok(accounts) = node.wallets.get_accounts_of_wallet(&wallet_id) {
            let block_transaction = node.store.tx_begin_read();

            println!("{:?}", accounts);

            for account in accounts {
                if let Some(info) = node.ledger.any().get_account(&block_transaction, &account) {
                    if info.modified >= modified_since {
                        let mut representative_opt = None;
                        let mut weight_opt = None;
                        let mut receivable_opt = None;

                        if representative {
                            representative_opt = Some(info.representative.as_account());
                        }

                        if weight {
                            weight_opt = Some(node.ledger.weight_exact(&block_transaction, account.into()));
                        }

                        if receivable {
                            let account_receivable = node.ledger.account_receivable(&block_transaction, &account, false);
                            receivable_opt = Some(account_receivable);
                        }

                        let entry = AccountInfo::new(
                            info.head,
                            info.open_block,
                            node.ledger.representative_block_hash(&block_transaction, &info.head),
                            info.balance,
                            info.modified,
                            info.block_count,
                            representative_opt,
                            weight_opt,
                            receivable_opt,
                            receivable_opt
                        );

                        accounts_json.insert(account, entry);
                    }
                }
                else {
                    println!("!!!!!!!!!!!!");
                }
            }

            to_string_pretty(&WalletLedgerDto { accounts: accounts_json }).unwrap()
        } else {
            to_string_pretty(&ErrorDto::new("Failed to get accounts".to_string())).unwrap()
        }
    }
    else {
        to_string_pretty(&ErrorDto::new("RPC control is disabled".to_string())).unwrap()
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
    fn wallet_ledger() {
        let mut system = System::new();
        let node = system.build_node().finish();
        let keys = KeyPair::new();
        let send_amount = Amount::from(100);
        let open_hash = setup_test_environment(node.clone(), keys.clone(), send_amount);

        let wallet_id = WalletId::zero();
        node.wallets.create(wallet_id);
        node.wallets.insert_adhoc2(&wallet_id, &keys.private_key(), true).unwrap();

        let wallet = wallet_id;
        let representative = Some(true);
        let weight = Some(true);
        let receivable = Some(true);
        let modified_since = None;

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let result = node.tokio.block_on(async {
            rpc_client.wallet_ledger(wallet, representative, weight, receivable, modified_since).await.unwrap()
        });

        let accounts = result.accounts;

        assert_eq!(accounts.len(), 1);
        let (account, info) = accounts.iter().next().unwrap();
        assert_eq!(*account, keys.account());
        assert_eq!(info.frontier, BlockHash::from(open_hash));
        assert_eq!(info.open_block, BlockHash::from(open_hash));
        assert_eq!(info.representative_block, BlockHash::from(open_hash));
        assert_eq!(info.balance, send_amount);
        assert!(info.modified_timestamp > 0);
        assert_eq!(info.block_count, 1);
        assert_eq!(info.weight, Some(send_amount));
        assert_eq!(info.pending, Some(Amount::zero()));
        assert_eq!(info.receivable, Some(Amount::zero()));
        assert_eq!(info.representative, Some(keys.account()));

        // Test without optional values
        let result_without_optional = node.tokio.block_on(async {
            rpc_client.wallet_ledger(wallet, None, None, None, None).await.unwrap()
        });

        let accounts_without_optional = result_without_optional.accounts;
        let (_, info_without_optional) = accounts_without_optional.iter().next().unwrap();
        assert!(info_without_optional.weight.is_none());
        assert!(info_without_optional.pending.is_none());
        assert!(info_without_optional.receivable.is_none());
        assert!(info_without_optional.representative.is_none());

        server.abort();
    }
}