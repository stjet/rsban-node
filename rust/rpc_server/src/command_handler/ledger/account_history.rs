use crate::command_handler::RpcCommandHandler;
use rsnano_core::{Account, Amount, Block, BlockEnum, BlockHash, BlockSubType};
use rsnano_rpc_messages::{AccountHistoryArgs, AccountHistoryDto, HistoryEntry};

impl RpcCommandHandler {
    pub(crate) fn account_history(&self, args: AccountHistoryArgs) -> AccountHistoryDto {
        let tx = self.node.store.tx_begin_read();
        let mut history = Vec::new();
        let reverse = args.reverse.unwrap_or(false);
        let mut hash = if reverse {
            self.node
                .ledger
                .any()
                .get_account(&tx, &args.account)
                .unwrap_or_default()
                .open_block
        } else {
            args.head.unwrap_or_else(|| {
                self.node
                    .ledger
                    .any()
                    .account_head(&tx, &args.account)
                    .unwrap_or_default()
            })
        };
        let mut count = args.count;
        let mut offset = args.offset.unwrap_or(0);
        let raw = args.raw.unwrap_or(false);
        let account_filter = args.account_filter.clone();

        while let Some(block) = self.node.ledger.get_block(&tx, &hash) {
            if offset > 0 {
                offset -= 1;
            } else if count > 0 {
                if let Some(entry) = self.create_history_entry(&block, &hash, raw, &account_filter)
                {
                    history.push(entry);
                    count -= 1;
                }
            } else {
                break;
            }

            hash = if !reverse {
                block.previous()
            } else {
                let a = self
                    .node
                    .ledger
                    .any()
                    .block_successor(&tx, &hash)
                    .unwrap_or_default();
                a
            };

            if hash.is_zero() {
                break;
            }
        }

        //if reverse {
        //history.reverse();
        //}

        let next = if !hash.is_zero() { Some(hash) } else { None };

        let previous = if !history.is_empty() {
            Some(if reverse {
                history.last().unwrap().hash
            } else {
                history.first().unwrap().hash
            })
        } else {
            None
        };

        AccountHistoryDto {
            account: args.account,
            history,
            previous,
            next,
        }
    }

    fn create_history_entry(
        &self,
        block: &BlockEnum,
        hash: &BlockHash,
        raw: bool,
        account_filter: &Option<Vec<Account>>,
    ) -> Option<HistoryEntry> {
        let transaction = self.node.store.tx_begin_read();
        let confirmed = self
            .node
            .ledger
            .confirmed()
            .block_exists_or_pruned(&transaction, hash);
        let local_timestamp = block.sideband().unwrap().timestamp;
        let height = block.sideband().unwrap().height;

        let (block_type, account, amount) = match block {
            BlockEnum::LegacySend(send_block) => {
                let amount = self
                    .node
                    .ledger
                    .any()
                    .block_amount(&transaction, hash)
                    .unwrap_or_default();
                let destination = *send_block.destination();
                if account_filter
                    .as_ref()
                    .map_or(false, |filter| !filter.contains(&destination))
                {
                    return None;
                }
                (BlockSubType::Send, destination, amount)
            }
            BlockEnum::LegacyReceive(receive_block) => {
                let amount = self
                    .node
                    .ledger
                    .any()
                    .block_amount(&transaction, hash)
                    .unwrap_or_default();
                let source_account = self
                    .node
                    .ledger
                    .any()
                    .block_account(&transaction, &receive_block.source())
                    .unwrap_or_default();
                if account_filter
                    .as_ref()
                    .map_or(false, |filter| !filter.contains(&source_account))
                {
                    return None;
                }
                (BlockSubType::Receive, source_account, amount)
            }
            BlockEnum::LegacyOpen(open_block) => {
                let (amount, source_account) = if open_block.source().as_bytes()
                    == self.node.ledger.constants.genesis_account.as_bytes()
                {
                    (
                        self.node.ledger.constants.genesis_amount,
                        self.node.ledger.constants.genesis_account,
                    )
                } else {
                    let amount = self
                        .node
                        .ledger
                        .any()
                        .block_amount(&transaction, hash)
                        .unwrap_or_default();
                    let source_account = self
                        .node
                        .ledger
                        .any()
                        .block_account(&transaction, &open_block.source())
                        .unwrap_or_default();
                    if account_filter
                        .as_ref()
                        .map_or(false, |filter| !filter.contains(&source_account))
                    {
                        return None;
                    } else {
                        (amount, source_account)
                    }
                };
                (BlockSubType::Receive, source_account, amount)
            }
            BlockEnum::LegacyChange(_) => {
                if raw {
                    (BlockSubType::Change, Account::default(), Amount::zero())
                } else {
                    return None; // Skip change blocks if not raw
                }
            }
            BlockEnum::State(state_block) => {
                if state_block.previous().is_zero() {
                    // Open block
                    let source_account = self
                        .node
                        .ledger
                        .any()
                        .block_account(&transaction, &state_block.link().into())
                        .unwrap_or_default();
                    if account_filter
                        .as_ref()
                        .map_or(false, |filter| !filter.contains(&source_account))
                    {
                        return None;
                    }
                    (BlockSubType::Receive, source_account, state_block.balance())
                } else {
                    let previous_balance = self
                        .node
                        .ledger
                        .any()
                        .block_balance(&transaction, &state_block.previous())
                        .unwrap_or_default();
                    if state_block.balance() < previous_balance {
                        // Send block
                        let destination = state_block.link().into();
                        if account_filter
                            .as_ref()
                            .map_or(false, |filter| !filter.contains(&destination))
                        {
                            return None;
                        }
                        (
                            BlockSubType::Send,
                            destination,
                            previous_balance - state_block.balance(),
                        )
                    } else if state_block.link().is_zero() {
                        // Change block
                        if raw {
                            (BlockSubType::Change, Account::default(), Amount::zero())
                        } else {
                            return None; // Skip change blocks if not raw
                        }
                    } else {
                        // Receive block
                        let source_account = self
                            .node
                            .ledger
                            .any()
                            .block_account(&transaction, &state_block.link().into())
                            .unwrap_or_default();
                        if account_filter
                            .as_ref()
                            .map_or(false, |filter| !filter.contains(&source_account))
                        {
                            return None;
                        }
                        (
                            BlockSubType::Receive,
                            source_account,
                            state_block.balance() - previous_balance,
                        )
                    }
                }
            }
        };

        Some(HistoryEntry {
            block_type,
            account,
            amount,
            local_timestamp,
            height,
            hash: *hash,
            confirmed,
            work: if raw { Some(block.work().into()) } else { None },
            signature: if raw {
                Some(block.block_signature().clone())
            } else {
                None
            },
        })
    }
}
