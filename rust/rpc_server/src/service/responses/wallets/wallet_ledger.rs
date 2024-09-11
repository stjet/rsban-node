use std::sync::Arc;
use rsnano_core::Account;
use rsnano_node::node::Node;
use rsnano_rpc_messages::{AccountInfo, ErrorDto, WalletLedgerArgs, WalletLedgerDto};
use serde_json::to_string_pretty;
use std::collections::HashMap;

pub async fn wallet_ledger(node: Arc<Node>, enable_control: bool, args: WalletLedgerArgs) -> String {
    if enable_control {
        //let representative = args.representative.unwrap_or(false);
        //let weight = args.weight.unwrap_or(false);
        let receivable = args.receivable.unwrap_or(args.receivable.unwrap_or(false));
        let modified_since = args.modified_since.unwrap_or(0);

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

                        accounts_json.insert(account, entry);
                    }
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