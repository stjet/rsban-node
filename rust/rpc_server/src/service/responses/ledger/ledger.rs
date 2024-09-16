use std::{collections::HashMap, sync::Arc};
use rsnano_core::{Account, AccountInfo, Amount};
use rsnano_node::node::Node;
use rsnano_rpc_messages::{LedgerAccountInfo, ErrorDto, LedgerArgs, LedgerDto};
use serde_json::to_string_pretty;

pub async fn ledger(node: Arc<Node>, enable_control: bool, args: LedgerArgs) -> String {
    if !enable_control {
        return to_string_pretty(&ErrorDto::new("RPC control is disabled".to_string())).unwrap();
    }

    let receivable = args.receivable.unwrap_or(false);
    let modified_since = args.modified_since.unwrap_or(0);
    let sorting = args.sorting.unwrap_or(false);
    let threshold = args.threshold.unwrap_or(Amount::zero());
    let count = args.count.unwrap_or(std::u64::MAX);
    let representative = args.representative.unwrap_or(false);
    let weight = args.weight.unwrap_or(false);

    let mut accounts_json: HashMap<Account, LedgerAccountInfo> = HashMap::new();
    let block_transaction = node.store.tx_begin_read();

    // Function to process an account
    let process_account = |account: Account, info: &AccountInfo, accounts_json: &mut HashMap<Account, LedgerAccountInfo>| {
        if info.modified >= modified_since {
            let mut representative_opt: Option<Account> = None;
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

            let entry = LedgerAccountInfo::new(
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

            let total_balance = info.balance + receivable_opt.unwrap_or(Amount::zero());
            if total_balance >= threshold {
                accounts_json.insert(account, entry);
            }
        }
    };

    let mut start_account = args.account;

    if !sorting {
        // Iterate through accounts
        while let Some(info) = node.store.account.get(&block_transaction, &start_account) {
            process_account(start_account, &info, &mut accounts_json);
            if accounts_json.len() >= count as usize {
                break;
            }
            start_account = node.store.account.iter(&block_transaction).next().unwrap().0;
        }
    } else {
        // Sorting implementation
        let mut ledger_l: Vec<(Amount, Account)> = Vec::new();
        while let Some(info) = node.store.account.get(&block_transaction, &start_account) {
            if info.modified >= modified_since {
                ledger_l.push((info.balance, start_account));
            }
            start_account = node.store.account.iter(&block_transaction).next().unwrap().0;
        }
        ledger_l.sort_by(|a, b| b.0.cmp(&a.0));
        for (_, account) in ledger_l {
            if let Some(info) = node.store.account.get(&block_transaction, &account) {
                process_account(account, &info, &mut accounts_json);
                if accounts_json.len() >= count as usize {
                    break;
                }
            }
        }
    }

    to_string_pretty(&LedgerDto { accounts: accounts_json }).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_core::{Amount, Block, BlockEnum, KeyPair, StateBlock};
    use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH};
    use test_helpers::System;
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;

    fn setup_test_environment(node: Arc<Node>) -> (KeyPair, StateBlock, StateBlock) {
        let keys = KeyPair::new();
        let genesis_balance = Amount::MAX;
        let send_amount = genesis_balance - Amount::from(100);
        let remaining_balance = genesis_balance - send_amount;

        let send_block = StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            *DEV_GENESIS_HASH,
            (*DEV_GENESIS_ACCOUNT).into(),
            remaining_balance,
            (*DEV_GENESIS_ACCOUNT).into(),
            &keys,
            node.work_generate_dev((*DEV_GENESIS_HASH).into()));

        node.process_active(BlockEnum::State(send_block.clone()));

        let open_block = StateBlock::new(
            keys.account(),
            *DEV_GENESIS_HASH,
            (*DEV_GENESIS_ACCOUNT).into(),
            send_amount,
            keys.account().into(),
            &keys,
            node.work_generate_dev(keys.account().into()),
        );

        node.process_active(BlockEnum::State(open_block.clone()));

        (keys, send_block, open_block)
    }

    #[test]
    fn test_ledger_basic_info() {
        let mut system = System::new();
        let node = system.build_node().finish();
        let (rpc_client, _server) = setup_rpc_client_and_server(node.clone(), true);

        let (keys, send_block, open_block) = setup_test_environment(node.clone());

        let send_amount = Amount::MAX - Amount::from(100);

        let result = node.tokio.block_on(async {
            rpc_client
                .ledger(
                    keys.account(),
                    Some(1), // sorting
                    Some(true),    // count
                    Some(true), // representative
                    Some(true), // weight
                    None, // pending
                    None,       // modified_since
                    None,       // threshold
                )
                .await
                .unwrap()
        });

        let accounts = result.accounts;
        assert_eq!(accounts.len(), 1);

        println!("{:?}", accounts);
        println!("{:?}", keys.account());

        /*let account_info = accounts.get(&keys.account()).unwrap();
        assert_eq!(account_info.frontier, open_block.hash());
        assert_eq!(account_info.open_block, open_block.hash());
        assert_eq!(account_info.representative_block, open_block.hash());
        assert_eq!(account_info.balance, send_amount);
        assert_eq!(account_info.block_count, 1);
        assert_eq!(account_info.representative, Some(*DEV_GENESIS_ACCOUNT));
        assert_eq!(account_info.weight, Some(Amount::zero()));
        assert_eq!(account_info.pending, Some(Amount::zero()));
        assert_eq!(account_info.receivable, Some(Amount::zero()));*/
    }

    /*#[test]
    fn test_ledger_threshold() {
        let system = System::new();
        let node = system.build_node().finish();
        let (rpc_client, _server) = setup_rpc_client_and_server(node.clone(), true);

        setup_test_environment(node.clone());

        let remaining_balance = Amount::from(rsnano_core::GENESIS_AMOUNT) - Amount::from(rsnano_core::GENESIS_AMOUNT - 100);

        let result = node.tokio.block_on(async {
            rpc_client
                .ledger(
                    None,
                    Some(true),                 // sorting
                    Some(2),                    // count
                    None,                       // representative
                    None,                       // weight
                    None,                       // pending
                    None,                       // modified_since
                    Some(remaining_balance + 1), // threshold
                )
                .await
                .unwrap()
        });

        let accounts = result.accounts;
        assert_eq!(accounts.len(), 1);
        assert!(accounts.contains_key(&key.account()));
    }

    #[test]
    fn test_ledger_pending() {
        let system = System::new();
        let node = system.build_node().finish();
        let (rpc_client, _server) = setup_rpc_client_and_server(node.clone(), true);

        setup_test_environment(node.clone());

        let send_amount = Amount::from(rsnano_core::GENESIS_AMOUNT) - Amount::from(100);
        let send2_amount = Amount::from(50);
        let new_remaining_balance = Amount::from(rsnano_core::GENESIS_AMOUNT) - send_amount - send2_amount;

        let send2_block = StateBlock::new_send(
            &send_block.hash(),
            &key.account(),
            new_remaining_balance,
            &DEV_GENESIS_PUB_KEY,
            &DEV_GENESIS_KEY,
            node.work_pool.generate(&send_block.hash()).unwrap(),
        ).unwrap();

        node.process_active(&send2_block).unwrap();

        let result = node.tokio.block_on(async {
            rpc_client
                .ledger(
                    None,
                    Some(true),                        // sorting
                    Some(2),                           // count
                    None,                              // representative
                    None,                              // weight
                    Some(true),                        // pending
                    None,                              // modified_since
                    Some(send_amount + send2_amount),  // threshold
                )
                .await
                .unwrap()
        });

        let accounts = result.accounts;
        assert_eq!(accounts.len(), 1);
        let account_info = accounts.get(&key.account()).unwrap();
        assert_eq!(account_info.balance, send_amount);
        assert_eq!(account_info.pending, Some(send2_amount));
    }*/
}