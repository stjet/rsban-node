use crate::command_handler::RpcCommandHandler;
use rsnano_core::{Account, Amount};
use rsnano_rpc_messages::{LedgerAccountInfo, LedgerArgs, LedgerResponse};
use std::{collections::HashMap, u64};

impl RpcCommandHandler {
    pub(crate) fn ledger(&self, args: LedgerArgs) -> LedgerResponse {
        let count = args.count.unwrap_or(u64::MAX);
        let threshold = args.threshold.unwrap_or(Amount::zero());
        let start = args.account.unwrap_or_default();
        let modified_since = args.modified_since.unwrap_or(0);
        let sorting = args.sorting.unwrap_or(false);
        let representative = args.representative.unwrap_or(false);
        let weight = args.weight.unwrap_or(false);
        let pending = args.pending.unwrap_or(false);
        let receivable = args.receivable.unwrap_or(pending);

        let mut accounts: HashMap<Account, LedgerAccountInfo> = HashMap::new();
        let tx = self.node.store.tx_begin_read();

        if !sorting {
            // Simple
            for (account, info) in self.node.store.account.iter_range(&tx, start..) {
                if info.modified >= modified_since && (receivable || info.balance >= threshold) {
                    let receivable = if receivable {
                        let account_receivable =
                            self.node.ledger.account_receivable(&tx, &account, false);
                        if info.balance + account_receivable < threshold {
                            continue;
                        }
                        Some(account_receivable)
                    } else {
                        None
                    };

                    let entry = LedgerAccountInfo {
                        frontier: info.head,
                        open_block: info.open_block,
                        representative_block: self
                            .node
                            .ledger
                            .representative_block_hash(&tx, &info.head),
                        balance: info.balance,
                        modified_timestamp: info.modified,
                        block_count: info.block_count,
                        representative: representative.then(|| info.representative.into()),
                        weight: weight.then(|| self.node.ledger.weight_exact(&tx, account.into())),
                        pending: receivable,
                        receivable,
                    };
                    accounts.insert(account, entry);
                    if accounts.len() >= count as usize {
                        break;
                    }
                }
            }
        } else {
            // Sorting
            let mut ledger: Vec<(Amount, Account)> = Vec::new();
            for (account, info) in self.node.store.account.iter_range(&tx, start..) {
                if info.modified >= modified_since {
                    ledger.push((info.balance, account));
                }
            }

            ledger.sort_by(|a, b| b.cmp(&a));

            for (_, account) in ledger {
                if let Some(info) = self.node.store.account.get(&tx, &account) {
                    if receivable || info.balance >= threshold {
                        let pending = if receivable {
                            let account_receivable =
                                self.node.ledger.account_receivable(&tx, &account, false);
                            if info.balance + account_receivable < threshold {
                                continue;
                            }
                            Some(account_receivable)
                        } else {
                            None
                        };

                        let entry = LedgerAccountInfo {
                            frontier: info.head,
                            open_block: info.open_block,
                            representative_block: self
                                .node
                                .ledger
                                .representative_block_hash(&tx, &info.head),
                            balance: info.balance,
                            modified_timestamp: info.modified,
                            block_count: info.block_count,
                            representative: representative.then(|| info.representative.into()),
                            weight: weight
                                .then(|| self.node.ledger.weight_exact(&tx, account.into())),
                            pending,
                            receivable: pending,
                        };
                        accounts.insert(account, entry);
                        if accounts.len() >= count as usize {
                            break;
                        }
                    }
                }
            }
        }

        LedgerResponse { accounts }
    }
}
