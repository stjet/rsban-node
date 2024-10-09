use rsnano_core::{Account, AccountInfo, Amount};
use rsnano_node::Node;
use rsnano_rpc_messages::{ErrorDto, LedgerAccountInfo, LedgerArgs, LedgerDto};
use serde_json::to_string_pretty;
use std::{collections::HashMap, sync::Arc};

pub async fn ledger(node: Arc<Node>, enable_control: bool, args: LedgerArgs) -> String {
    if !enable_control {
        return to_string_pretty(&ErrorDto::new("RPC control is disabled".to_string())).unwrap();
    }

    let account = args.account;
    let count = args.count.unwrap_or(std::u64::MAX);
    let representative = args.representative.unwrap_or(false);
    let weight = args.weight.unwrap_or(false);
    let pending = args.pending.unwrap_or(false);
    let receivable = args.receivable.unwrap_or(pending);
    let modified_since = args.modified_since.unwrap_or(0);
    let sorting = args.sorting.unwrap_or(false);
    let threshold = args.threshold.unwrap_or(Amount::zero());

    let mut accounts_json: HashMap<Account, LedgerAccountInfo> = HashMap::new();
    let block_transaction = node.store.tx_begin_read();

    let account_iter: Box<dyn Iterator<Item = (Account, AccountInfo)>> = match account {
        Some(acc) => Box::new(node.store.account.iter_range(&block_transaction, acc..)),
        None => Box::new(node.store.account.iter(&block_transaction)),
    };

    if !sorting {
        for (current_account, info) in account_iter {
            if info.modified >= modified_since {
                let account_receivable = if receivable {
                    node.ledger.account_receivable(&block_transaction, &current_account, false)
                } else {
                    Amount::zero()
                };
                let total_balance = info.balance + account_receivable;
                if total_balance >= threshold {
                    process_account(
                        node.clone(),
                        current_account,
                        &info,
                        representative,
                        weight,
                        receivable,            
                        &mut accounts_json,
                        account_receivable,
                    );
                }
            }
            if accounts_json.len() >= count as usize {
                break;
            }
        }
    } else {
        let mut ledger_l: Vec<(Amount, Account)> = Vec::new();
        match account {
            Some(acc) => {
                let mut iter = node.store.account.begin_account(&block_transaction, &acc);
                while let Some((current_account, info)) = iter.current() {
                    if info.modified >= modified_since {
                        ledger_l.push((info.balance, *current_account));
                    }
                    iter.next();
                }
            }
            None => {
                let iter = node.store.account.iter(&block_transaction);
                for (account, info) in iter {
                    if info.modified >= modified_since {
                        ledger_l.push((info.balance, account));
                    }
                }
            }
        }

        ledger_l.sort_by(|a, b| b.0.cmp(&a.0));
        for (_, account) in ledger_l {
            if let Some(info) = node.store.account.get(&block_transaction, &account) {
                let account_receivable = if receivable {
                    node.ledger.account_receivable(&block_transaction, &account, false)
                } else {
                    Amount::zero()
                };
                let total_balance = info.balance + account_receivable;
                if total_balance >= threshold {
                    process_account(
                        node.clone(),
                        account,
                        &info,
                        representative,
                        weight,
                        receivable,            
                        &mut accounts_json,
                        account_receivable,
                    );
                    if accounts_json.len() >= count as usize {
                        break;
                    }
                }
            }
        }
    }

    to_string_pretty(&LedgerDto {
        accounts: accounts_json,
    })
    .unwrap()
}

fn process_account(
    node: Arc<Node>,
    account: Account,
    info: &AccountInfo,
    representative: bool,
    weight: bool,
    pending: bool,
    accounts_json: &mut HashMap<Account, LedgerAccountInfo>,
    account_receivable: Amount,
) {
    let block_transaction = node.ledger.read_txn();
    let mut representative_opt = None;
    let mut weight_opt = None;
    let mut pending_opt = None;

    if representative {
        representative_opt = Some(info.representative);
    }
    if weight {
        weight_opt = Some(node.ledger.weight(&account.into()));
    }
    if pending {
        pending_opt = Some(account_receivable);
    }

    let entry = LedgerAccountInfo::new(
        info.head,
        info.open_block,
        node.ledger
            .representative_block_hash(&block_transaction, &info.head),
        info.balance,
        info.modified,
        info.block_count,
        representative_opt.map(|inner| inner.into()),
        weight_opt,
        pending_opt,     // Pending field
        pending_opt,     // Receivable field
    );
    accounts_json.insert(account, entry);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_core::{Amount, BlockEnum, BlockHash, KeyPair, StateBlock, DEV_GENESIS_KEY};
    use rsnano_ledger::{BlockStatus, DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
    use std::time::Duration;
    use test_helpers::{assert_timely_msg, setup_rpc_client_and_server, System};

    fn setup_test_environment(node: Arc<Node>) -> (KeyPair, BlockEnum, BlockEnum) {
        let keys = KeyPair::new();
        let rep_weight = Amount::MAX - Amount::raw(100);

        let send = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            *DEV_GENESIS_HASH,
            *DEV_GENESIS_PUB_KEY,
            Amount::MAX - rep_weight,
            keys.account().into(),
            &DEV_GENESIS_KEY,
            node.work_generate_dev((*DEV_GENESIS_HASH).into()),
        ));

        let status = node.process_local(send.clone()).unwrap();
        assert_eq!(status, BlockStatus::Progress);

        let open = BlockEnum::State(StateBlock::new(
            keys.account(),
            BlockHash::zero(),
            *DEV_GENESIS_PUB_KEY,
            rep_weight,
            send.hash().into(),
            &keys,
            node.work_generate_dev(keys.public_key().into()),
        ));

        let status = node.process_local(open.clone()).unwrap();
        assert_eq!(status, BlockStatus::Progress);

        (keys, send, open)
    }

    #[test]
    fn test_ledger() {
        let mut system = System::new();
        let node = system.build_node().finish();
        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let (keys, send, open) = setup_test_environment(node.clone());

        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let result = node.runtime.block_on(async {
            rpc_client
                .ledger(
                    None,       
                    Some(1),    
                    None,  
                    None,     
                    None,       
                    None,       
                    None,       
                    Some(true), 
                    None,      
                )
                .await
                .unwrap()
        });

        let accounts = result.accounts;
        assert_eq!(accounts.len(), 1);

        for (account, info) in accounts {
            assert_eq!(keys.account(), account);
            assert_eq!(open.hash(), info.frontier);
            assert_eq!(open.hash(), info.open_block);
            assert_eq!(open.hash(), info.representative_block);
            assert_eq!(Amount::MAX - Amount::raw(100), info.balance);
            assert!(((time as i64) - (info.modified_timestamp as i64)).abs() < 5);
            assert_eq!(1, info.block_count);
            assert!(info.weight.is_none());
            assert!(info.pending.is_none());
            assert!(info.representative.is_none());
        }

        server.abort();
    }

    #[test]
    fn test_ledger_threshold() {
        let mut system = System::new();
        let node = system.build_node().finish();
        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let (keys, _, _) = setup_test_environment(node.clone());

        let genesis_balance = Amount::MAX;
        let result = node.runtime.block_on(async {
            rpc_client
                .ledger(
                    None,                                
                    Some(2),                                
                    None,  
                    None,                             
                    None,                            
                    None,                                 
                    None,                                   
                    Some(true),                     
                    Some(genesis_balance - Amount::raw(100)), 
                )
                .await
                .unwrap()
        });

        let accounts = result.accounts;
        assert_eq!(accounts.len(), 1);
        assert!(accounts.contains_key(&keys.account()));

        server.abort();
    }

    #[test]
    fn test_ledger_pending() {
        let mut system = System::new();
        let node = system.build_node().finish();
        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let (keys, send_block, _) = setup_test_environment(node.clone());

        let send_amount = Amount::MAX - Amount::raw(100);
        let send2_amount = Amount::raw(50);
        let new_remaining_balance = Amount::MAX - send_amount - send2_amount;

        let send2_block = StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            send_block.hash(),
            keys.account().into(),
            new_remaining_balance,
            keys.account().into(),
            &DEV_GENESIS_KEY,
            node.work_generate_dev(send_block.hash().into()),
        );

        let status = node.process_local(BlockEnum::State(send2_block.clone())).unwrap();
        assert_eq!(status, BlockStatus::Progress);

        let result = node.runtime.block_on(async {
            rpc_client
                .ledger(
                    None,                              
                    Some(2),                          
                    None,                              
                    None,                          
                    Some(true),                    
                    None,                          
                    None,                          
                    None,                              
                    Some(send_amount + send2_amount), 
                )
                .await
                .unwrap()
        });

        let accounts = result.accounts;
        assert_eq!(accounts.len(), 1);
        let account_info = accounts.get(&keys.account()).unwrap();
        assert_eq!(account_info.balance, send_amount);
        assert_eq!(account_info.pending, Some(send2_amount));

        server.abort();
    }
}