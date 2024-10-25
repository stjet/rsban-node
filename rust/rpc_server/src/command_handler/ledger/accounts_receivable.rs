use crate::command_handler::RpcCommandHandler;
use itertools::Itertools;
use rsnano_core::{Account, Amount, BlockHash};
use rsnano_rpc_messages::{AccountsReceivableArgs, ReceivableDto, SourceInfo};
use std::collections::HashMap;

impl RpcCommandHandler {
    pub(crate) fn accounts_receivable(&self, args: AccountsReceivableArgs) -> ReceivableDto {
        let transaction = self.node.store.tx_begin_read();
        let count = args.count;
        let threshold = args.threshold.unwrap_or(Amount::zero());
        let source = args.source.unwrap_or(false);
        let include_only_confirmed = args.include_only_confirmed.unwrap_or(false);
        let sorting = args.sorting.unwrap_or(false);
        let simple = threshold.is_zero() && !source && !sorting;

        let result = if simple {
            let mut blocks: HashMap<Account, Vec<BlockHash>> = HashMap::new();
            for account in args.accounts {
                let mut receivable_hashes = Vec::new();
                let mut iterator = self.node.ledger.any().account_receivable_upper_bound(
                    &transaction,
                    account,
                    BlockHash::zero(),
                );
                while let Some((key, _info)) = iterator.next() {
                    if receivable_hashes.len() >= count as usize {
                        break;
                    }
                    if include_only_confirmed
                        && !self
                            .node
                            .ledger
                            .confirmed()
                            .block_exists_or_pruned(&transaction, &key.send_block_hash)
                    {
                        continue;
                    }
                    receivable_hashes.push(key.send_block_hash);
                }
                if !receivable_hashes.is_empty() {
                    blocks.insert(account, receivable_hashes);
                }
            }
            ReceivableDto::Blocks { blocks }
        } else if source {
            let mut blocks: HashMap<Account, HashMap<BlockHash, SourceInfo>> = HashMap::new();
            for account in args.accounts {
                let mut receivable_info = HashMap::new();
                for current in self.node.ledger.any().account_receivable_upper_bound(
                    &transaction,
                    account,
                    BlockHash::zero(),
                ) {
                    if receivable_info.len() >= count as usize {
                        break;
                    }
                    let (key, info) = current;
                    if include_only_confirmed
                        && !self
                            .node
                            .ledger
                            .confirmed()
                            .block_exists_or_pruned(&transaction, &key.send_block_hash)
                    {
                        continue;
                    }
                    if info.amount < threshold {
                        continue;
                    }
                    receivable_info.insert(
                        key.send_block_hash,
                        SourceInfo {
                            amount: info.amount,
                            source: info.source,
                        },
                    );
                }
                if !receivable_info.is_empty() {
                    blocks.insert(account, receivable_info);
                }
            }
            if sorting {
                for (_, receivable_info) in blocks.iter_mut() {
                    *receivable_info = receivable_info
                        .drain()
                        .sorted_by(|a, b| b.1.amount.cmp(&a.1.amount))
                        .collect();
                }
            }
            ReceivableDto::Source { blocks }
        } else {
            let mut blocks: HashMap<Account, HashMap<BlockHash, Amount>> = HashMap::new();
            for account in args.accounts {
                let mut receivable_amounts = HashMap::new();
                for current in self.node.ledger.any().account_receivable_upper_bound(
                    &transaction,
                    account,
                    BlockHash::zero(),
                ) {
                    if receivable_amounts.len() >= count as usize {
                        break;
                    }
                    let (key, info) = current;
                    if include_only_confirmed
                        && !self
                            .node
                            .ledger
                            .confirmed()
                            .block_exists_or_pruned(&transaction, &key.send_block_hash)
                    {
                        continue;
                    }
                    if info.amount < threshold {
                        continue;
                    }
                    receivable_amounts.insert(key.send_block_hash, info.amount);
                }
                if !receivable_amounts.is_empty() {
                    blocks.insert(account, receivable_amounts);
                }
            }
            if sorting {
                for (_, receivable_amounts) in blocks.iter_mut() {
                    *receivable_amounts = receivable_amounts
                        .drain()
                        .collect::<Vec<_>>()
                        .into_iter()
                        .sorted_by(|a, b| b.1.cmp(&a.1))
                        .collect();
                }
            }
            ReceivableDto::Threshold { blocks }
        };

        result
    }
}
