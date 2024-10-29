use crate::command_handler::RpcCommandHandler;
use indexmap::IndexMap;
use rsnano_core::{Account, Amount, BlockHash};
use rsnano_rpc_messages::{
    ReceivableArgs, ReceivableDto, ReceivableSimple, ReceivableSource, ReceivableThreshold,
    SourceInfo,
};

impl RpcCommandHandler {
    pub(crate) fn receivable(&self, args: ReceivableArgs) -> ReceivableDto {
        let transaction = self.node.store.tx_begin_read();
        let receivables = self.node.ledger.any().account_receivable_upper_bound(
            &transaction,
            args.account,
            BlockHash::zero(),
        );

        let mut blocks_source: IndexMap<Account, IndexMap<BlockHash, SourceInfo>> = IndexMap::new();
        let mut blocks_threshold: IndexMap<Account, IndexMap<BlockHash, Amount>> = IndexMap::new();
        let mut blocks_default: IndexMap<Account, Vec<BlockHash>> = IndexMap::new();

        let mut account_blocks_source: Vec<(BlockHash, SourceInfo)> = Vec::new();
        let mut account_blocks_threshold: Vec<(BlockHash, Amount)> = Vec::new();
        let mut account_blocks: Vec<BlockHash> = Vec::new();

        for (key, info) in receivables {
            if args.include_only_confirmed.unwrap_or(true)
                && !self
                    .node
                    .ledger
                    .confirmed()
                    .block_exists_or_pruned(&transaction, &key.send_block_hash)
            {
                continue;
            }

            if let Some(threshold) = args.threshold {
                if info.amount < threshold {
                    continue;
                }
            }

            if args.source.unwrap_or(false) {
                account_blocks_source.push((
                    key.send_block_hash,
                    SourceInfo {
                        amount: info.amount,
                        source: info.source,
                    },
                ));
            } else if args.threshold.is_some() {
                account_blocks_threshold.push((key.send_block_hash, info.amount));
            } else {
                account_blocks.push(key.send_block_hash);
            }

            if account_blocks.len() >= args.count as usize
                || account_blocks_threshold.len() >= args.count as usize
                || account_blocks_source.len() >= args.count as usize
            {
                break;
            }
        }

        if args.sorting.unwrap_or(false) {
            if args.source.unwrap_or(false) {
                account_blocks_source.sort_by(|a, b| b.1.amount.cmp(&a.1.amount));
            } else if args.threshold.is_some() {
                account_blocks_threshold.sort_by(|a, b| b.1.cmp(&a.1));
            }
            // Note: We don't sort account_blocks as it's only used for the simple case
        }

        // Apply offset and limit
        let offset = 0; //args.offset.unwrap_or(0) as usize;
        let count = args.count as usize;

        let receivable_dto = if args.source.unwrap_or(false) {
            blocks_source.insert(
                args.account,
                account_blocks_source
                    .into_iter()
                    .skip(offset)
                    .take(count)
                    .collect::<IndexMap<_, _>>(),
            );
            ReceivableDto::Source(ReceivableSource {
                blocks: blocks_source,
            })
        } else if args.threshold.is_some() {
            blocks_threshold.insert(
                args.account,
                account_blocks_threshold
                    .into_iter()
                    .skip(offset)
                    .take(count)
                    .collect(),
            );
            ReceivableDto::Threshold(ReceivableThreshold {
                blocks: blocks_threshold,
            })
        } else {
            blocks_default.insert(
                args.account,
                account_blocks
                    .into_iter()
                    .skip(offset)
                    .take(count)
                    .collect(),
            );
            ReceivableDto::Simple(ReceivableSimple {
                blocks: blocks_default,
            })
        };

        receivable_dto
    }
}
