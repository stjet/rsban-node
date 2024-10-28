use crate::command_handler::RpcCommandHandler;
use anyhow::{anyhow, bail};
use rsnano_core::{Account, Block, BlockEnum, BlockHash};
use rsnano_ledger::Ledger;
use rsnano_rpc_messages::{
    AccountHistoryArgs, AccountHistoryResponse, BlockSubTypeDto, BlockTypeDto, HistoryEntry,
};

impl RpcCommandHandler {
    pub(crate) fn account_history(
        &self,
        args: AccountHistoryArgs,
    ) -> anyhow::Result<AccountHistoryResponse> {
        let helper = AccountHistoryHelper::new(&self.node.ledger, args);
        helper.account_history()
    }
}

struct AccountHistoryHelper<'a> {
    ledger: &'a Ledger,
    accounts_to_filter: Vec<Account>,
    reverse: bool,
    offset: u64,
    head: Option<BlockHash>,
    account: Option<Account>,
    output_raw: bool,
    count: u64,
}

impl<'a> AccountHistoryHelper<'a> {
    fn new(ledger: &'a Ledger, args: AccountHistoryArgs) -> Self {
        Self {
            ledger,
            accounts_to_filter: args.account_filter.unwrap_or_default(),
            reverse: args.reverse.unwrap_or(false),
            offset: args.offset.unwrap_or_default(),
            account: args.account,
            output_raw: args.raw.unwrap_or(false),
            count: args.count,
            head: args.head,
        }
    }

    pub(crate) fn account_history(mut self) -> anyhow::Result<AccountHistoryResponse> {
        let tx = self.ledger.read_txn();
        let (account, mut hash) = if let Some(head) = &self.head {
            let account = self
                .ledger
                .any()
                .block_account(&tx, head)
                .ok_or_else(|| anyhow!(RpcCommandHandler::BLOCK_NOT_FOUND))?;
            (account, *head)
        } else {
            let account = self
                .account
                .ok_or_else(|| anyhow!("account argument missing"))?;

            let hash = if self.reverse {
                let info = self
                    .ledger
                    .any()
                    .get_account(&tx, &account)
                    .ok_or_else(|| anyhow!("Account not found"))?;
                info.open_block
            } else {
                self.ledger
                    .any()
                    .account_head(&tx, &account)
                    .ok_or_else(|| anyhow!("Account not found"))?
            };

            (account, hash)
        };

        let mut history = Vec::new();

        let mut next_block = self.ledger.any().get_block(&tx, &hash);
        while let Some(block) = next_block {
            if self.count == 0 {
                break;
            }

            if self.offset > 0 {
                self.offset -= 1;
            } else {
                let sideband = block.sideband().unwrap();
                let mut entry = HistoryEntry {
                    block_type: None,
                    amount: None,
                    account: None,
                    local_timestamp: sideband.timestamp,
                    height: sideband.height,
                    hash,
                    confirmed: self.ledger.confirmed().block_exists_or_pruned(&tx, &hash),
                    work: if self.output_raw {
                        Some(block.work().into())
                    } else {
                        None
                    },
                    signature: if self.output_raw {
                        Some(block.block_signature().clone())
                    } else {
                        None
                    },
                    representative: None,
                    previous: None,
                    balance: None,
                    source: None,
                    opened: None,
                    destination: None,
                    link: None,
                    subtype: None,
                };
                let mut skip_this_entry = false;
                match &block {
                    BlockEnum::LegacySend(b) => {
                        entry.block_type = Some(BlockTypeDto::Send);
                        entry.account = Some(account);
                        if let Some(amount) = self.ledger.any().block_amount(&tx, &hash) {
                            entry.amount = Some(amount);
                        } else {
                            entry.destination = Some(account);
                            entry.balance = Some(b.balance());
                            entry.previous = Some(b.previous());
                        }
                    }
                    BlockEnum::LegacyReceive(b) => {
                        entry.block_type = Some(BlockTypeDto::Receive);
                        if let Some(amount) = self.ledger.any().block_amount(&tx, &hash) {
                            if let Some(source_account) =
                                self.ledger.any().block_account(&tx, &b.source())
                            {
                                entry.account = Some(source_account);
                            }
                            entry.amount = Some(amount);
                        }
                        if self.output_raw {
                            entry.source = Some(b.source());
                            entry.previous = Some(b.previous());
                        }
                    }
                    BlockEnum::LegacyOpen(b) => {
                        if self.output_raw {
                            entry.block_type = Some(BlockTypeDto::Open);
                            entry.representative = Some(b.hashables.representative.into());
                            entry.source = Some(b.source());
                            entry.opened = Some(b.account());
                        } else {
                            // Report opens as a receive
                            entry.block_type = Some(BlockTypeDto::Receive);
                        }

                        if b.source() != self.ledger.constants.genesis_account.into() {
                            if let Some(amount) = self.ledger.any().block_amount(&tx, &hash) {
                                entry.account = self.ledger.any().block_account(&tx, &b.source());
                                entry.amount = Some(amount);
                            }
                        } else {
                            entry.account = Some(self.ledger.constants.genesis_account);
                            entry.amount = Some(self.ledger.constants.genesis_amount);
                        }
                    }
                    BlockEnum::LegacyChange(b) => {
                        if self.output_raw {
                            entry.block_type = Some(BlockTypeDto::Change);
                            entry.representative = Some(b.mandatory_representative().into());
                            entry.previous = Some(b.previous());
                        } else {
                            skip_this_entry = true;
                        }
                    }
                    BlockEnum::State(b) => {
                        if self.output_raw {
                            entry.block_type = Some(BlockTypeDto::State);
                            entry.representative = Some(b.mandatory_representative().into());
                            entry.link = Some(b.link());
                            entry.balance = Some(b.balance());
                            entry.previous = Some(b.previous());
                        }

                        let balance = b.balance();
                        let previous_balance_raw =
                            self.ledger.any().block_balance(&tx, &b.previous());
                        let previous_balance = previous_balance_raw.unwrap_or_default();
                        if !b.previous().is_zero() && previous_balance_raw.is_none() {
                            // If previous hash is non-zero and we can't query the balance, e.g. it's pruned, we can't determine the block type
                            if self.output_raw {
                                entry.subtype = Some(BlockSubTypeDto::Unknown);
                            } else {
                                entry.block_type = Some(BlockTypeDto::Unknown);
                            }
                        } else if balance < previous_balance {
                            if self.should_ignore_account(&b.link().into()) {
                                skip_this_entry = !self.output_raw;
                            } else {
                                if self.output_raw {
                                    entry.subtype = Some(BlockSubTypeDto::Send);
                                } else {
                                    entry.block_type = Some(BlockTypeDto::Send);
                                }
                                entry.account = Some(b.link().into());
                                entry.amount = Some(previous_balance - b.balance());
                            }
                        } else {
                            if b.link().is_zero() {
                                if self.output_raw && self.accounts_to_filter.is_empty() {
                                    entry.subtype = Some(BlockSubTypeDto::Change);
                                } else {
                                    skip_this_entry = !self.output_raw;
                                }
                            } else if balance == previous_balance
                                && self.ledger.is_epoch_link(&b.link())
                            {
                                if self.output_raw && self.accounts_to_filter.is_empty() {
                                    entry.subtype = Some(BlockSubTypeDto::Epoch);
                                    entry.account = self.ledger.epoch_signer(&b.link());
                                } else {
                                    skip_this_entry = !self.output_raw;
                                }
                            } else {
                                let source_account_opt =
                                    self.ledger.any().block_account(&tx, &b.link().into());
                                let source_account = source_account_opt.unwrap_or_default();

                                if source_account_opt.is_some()
                                    && self.should_ignore_account(&source_account)
                                {
                                    skip_this_entry = !self.output_raw;
                                } else {
                                    if self.output_raw {
                                        entry.subtype = Some(BlockSubTypeDto::Receive);
                                    } else {
                                        entry.block_type = Some(BlockTypeDto::Receive);
                                    }
                                    if source_account_opt.is_some() {
                                        entry.account = Some(source_account);
                                    }
                                    entry.amount = Some(balance - previous_balance);
                                }
                            }
                        }
                    }
                };
                if !skip_this_entry {
                    history.push(entry);
                    self.count -= 1;
                }
            }

            hash = if self.reverse {
                self.ledger
                    .any()
                    .block_successor(&tx, &hash)
                    .unwrap_or_default()
            } else {
                block.previous()
            };
            next_block = self.ledger.any().get_block(&tx, &hash);
        }

        let mut response = AccountHistoryResponse {
            account,
            history,
            previous: None,
            next: None,
        };

        if !hash.is_zero() {
            if self.reverse {
                response.next = Some(hash);
            } else {
                response.previous = Some(hash);
            }
        }

        Ok(response)
    }

    fn should_ignore_account(&self, account: &Account) -> bool {
        if self.accounts_to_filter.is_empty() {
            return false;
        }
        !self.accounts_to_filter.contains(account)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_ledger::DEV_GENESIS_ACCOUNT;
    use rsnano_node::Node;
    use rsnano_rpc_messages::{check_error, RpcCommand};
    use std::sync::Arc;

    #[tokio::test]
    async fn history_rpc_call() {
        let node = Arc::new(Node::new_null());
        let cmd_handler = RpcCommandHandler::new(node, true);
        let result = cmd_handler.handle(RpcCommand::account_history(
            AccountHistoryArgs::builder_for_account(Account::from(42), 3).build(),
        ));
        let error = check_error(&result).unwrap_err();
        assert_eq!(error, "Account not found");
    }
}
