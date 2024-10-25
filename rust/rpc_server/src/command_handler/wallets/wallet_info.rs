use crate::command_handler::RpcCommandHandler;
use rsnano_core::Amount;
use rsnano_rpc_messages::{WalletInfoDto, WalletRpcMessage};
use rsnano_store_lmdb::KeyType;

impl RpcCommandHandler {
    pub(crate) fn wallet_info(&self, args: WalletRpcMessage) -> anyhow::Result<WalletInfoDto> {
        let block_transaction = self.node.ledger.read_txn();
        let accounts = self.node.wallets.get_accounts_of_wallet(&args.wallet)?;

        let mut balance = Amount::zero();
        let mut receivable = Amount::zero();
        let mut count = 0u64;
        let mut block_count = 0u64;
        let mut cemented_block_count = 0u64;
        let mut deterministic_count = 0u64;
        let mut adhoc_count = 0u64;

        for account in accounts {
            if let Some(account_info) = self.node.ledger.account_info(&block_transaction, &account)
            {
                block_count += account_info.block_count;
                balance += account_info.balance;
            }

            if let Some(confirmation_info) = self
                .node
                .store
                .confirmation_height
                .get(&block_transaction, &account)
            {
                cemented_block_count += confirmation_info.height;
            }

            receivable += self
                .node
                .ledger
                .account_receivable(&block_transaction, &account, false);

            match self.node.wallets.key_type(args.wallet, &account.into()) {
                KeyType::Deterministic => deterministic_count += 1,
                KeyType::Adhoc => adhoc_count += 1,
                _ => (),
            }

            count += 1;
        }

        let deterministic_index = self
            .node
            .wallets
            .deterministic_index_get(&args.wallet)
            .unwrap();

        Ok(WalletInfoDto::new(
            balance,
            receivable,
            receivable,
            count,
            adhoc_count,
            deterministic_count,
            deterministic_index,
            block_count,
            cemented_block_count,
        ))
    }
}
