use crate::command_handler::RpcCommandHandler;
use indexmap::IndexMap;
use rsnano_core::{Amount, BlockHash};
use rsnano_rpc_messages::{
    AccountsReceivableResponse, AccountsReceivableSimple, AccountsReceivableSource,
    AccountsReceivableThreshold, SourceInfo, WalletReceivableArgs,
};

impl RpcCommandHandler {
    pub(crate) fn wallet_receivable(
        &self,
        args: WalletReceivableArgs,
    ) -> anyhow::Result<AccountsReceivableResponse> {
        let accounts = self.node.wallets.get_accounts_of_wallet(&args.wallet)?;

        let tx = self.node.ledger.read_txn();
        let mut block_source = IndexMap::new();
        let mut block_threshold = IndexMap::new();
        let mut block_default = IndexMap::new();

        for account in accounts {
            let mut account_blocks_source: IndexMap<BlockHash, SourceInfo> = IndexMap::new();
            let mut account_blocks_threshold: IndexMap<BlockHash, Amount> = IndexMap::new();
            let mut account_blocks_default: Vec<BlockHash> = Vec::new();
            for (key, info) in self
                .node
                .ledger
                .any()
                .account_receivable_upper_bound(&tx, account, BlockHash::zero())
                .take(args.count as usize)
            {
                if args.include_only_confirmed.unwrap_or(true)
                    && !self
                        .node
                        .ledger
                        .confirmed()
                        .block_exists_or_pruned(&tx, &key.send_block_hash)
                {
                    continue;
                }

                if let Some(threshold) = args.threshold {
                    if info.amount < threshold {
                        continue;
                    }
                }

                if args.source.unwrap_or(false) || args.min_version.unwrap_or(false) {
                    let source_info = SourceInfo {
                        amount: info.amount,
                        source: Some(info.source),
                        min_version: None,
                    };
                    account_blocks_source.insert(key.send_block_hash, source_info);
                } else if args.threshold.is_some() {
                    account_blocks_threshold.insert(key.send_block_hash, info.amount);
                } else {
                    account_blocks_default.push(key.send_block_hash);
                }
            }

            if !account_blocks_source.is_empty() {
                block_source.insert(account, account_blocks_source);
            }
            if !account_blocks_threshold.is_empty() {
                block_threshold.insert(account, account_blocks_threshold);
            }
            if !account_blocks_default.is_empty() {
                block_default.insert(account, account_blocks_default);
            }
        }

        let result = if args.source.unwrap_or(false) || args.min_version.unwrap_or(false) {
            AccountsReceivableResponse::Source(AccountsReceivableSource {
                blocks: block_source,
            })
        } else if args.threshold.is_some() {
            AccountsReceivableResponse::Threshold(AccountsReceivableThreshold {
                blocks: block_threshold,
            })
        } else {
            AccountsReceivableResponse::Simple(AccountsReceivableSimple {
                blocks: block_default,
            })
        };
        Ok(result)
    }
}
