use std::{collections::HashMap, sync::Arc};
use rsnano_core::Account;
use rsnano_node::node::Node;
use rsnano_rpc_messages::{AccountInfo, ErrorDto, LedgerArgs, LedgerDto};
use serde_json::to_string_pretty;

pub async fn ledger(node: Arc<Node>, enable_control: bool, args: LedgerArgs) -> String {
    if enable_control {
        let receivable = args.receivable.unwrap_or(false);
        let modified_since = args.modified_since.unwrap_or(0);
        let sorting = args.sorting.unwrap_or(false);
        let threshold = args.threshold;

        let wallet_id = args.wallet.clone();
        
        let mut accounts_json: HashMap<Account, AccountInfo> = HashMap::new();

        if let Ok(accounts) = node.wallets.get_accounts_of_wallet(&wallet_id) {
            let block_transaction = node.store.tx_begin_read();

            for account in accounts {
                if let Some(info) = node.ledger.any().get_account(&block_transaction, &account) {
                    if info.modified >= modified_since {
                        let mut representative_opt = None;
                        let mut weight_opt = None;
                        let mut receivable_opt = None;

                        if args.representative.is_some() {
                            representative_opt = Some(info.representative.as_account());
                        }

                        if args.weight.is_some() {
                            weight_opt = Some(node.ledger.weight_exact(&block_transaction, account.into()));
                        }

                        if args.receivable.is_some() {
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

                        if threshold.is_none() || entry.balance >= threshold.unwrap() {
                            accounts_json.insert(account, entry);
                        }
                    }
                }
            }

            if sorting {
                let mut sorted_accounts: Vec<_> = accounts_json.into_iter().collect();
                sorted_accounts.sort_by(|a, b| b.1.balance.cmp(&a.1.balance));
                accounts_json = sorted_accounts.into_iter().collect();
            }

            to_string_pretty(&LedgerDto { accounts: accounts_json }).unwrap()
        } else {
            to_string_pretty(&ErrorDto::new("Failed to get accounts".to_string())).unwrap()
        }
    }
    else {
        to_string_pretty(&ErrorDto::new("RPC control is disabled".to_string())).unwrap()
    }
}