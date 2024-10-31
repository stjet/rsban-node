use crate::command_handler::RpcCommandHandler;
use rsnano_core::Amount;
use rsnano_rpc_messages::{WalletInfoResponse, WalletRpcMessage};
use rsnano_store_lmdb::KeyType;

impl RpcCommandHandler {
    pub(crate) fn wallet_info(&self, args: WalletRpcMessage) -> anyhow::Result<WalletInfoResponse> {
        let accounts = self.node.wallets.decrypt(args.wallet)?;
        let mut balance = Amount::zero();
        let mut receivable = Amount::zero();
        let mut accounts_count = 0u64;
        let mut block_count = 0u64;
        let mut cemented_count = 0u64;
        let mut deterministic_count = 0u64;
        let mut adhoc_count = 0u64;
        let tx = self.node.ledger.read_txn();

        for (account, _priv_key) in accounts {
            let account = account.into();
            if let Some(account_info) = self.node.ledger.account_info(&tx, &account) {
                block_count += account_info.block_count;
                balance += account_info.balance;
            }

            if let Some(confirmation_info) = self.node.store.confirmation_height.get(&tx, &account)
            {
                cemented_count += confirmation_info.height;
            }

            receivable += self.node.ledger.account_receivable(&tx, &account, false);

            match self.node.wallets.key_type(args.wallet, &account.into()) {
                KeyType::Deterministic => deterministic_count += 1,
                KeyType::Adhoc => adhoc_count += 1,
                _ => {}
            }

            accounts_count += 1;
        }

        let deterministic_index = self
            .node
            .wallets
            .deterministic_index_get(&args.wallet)
            .unwrap();

        Ok(WalletInfoResponse {
            balance,
            receivable,
            pending: receivable,
            accounts_count,
            adhoc_count,
            deterministic_count,
            deterministic_index,
            accounts_block_count: block_count,
            accounts_cemented_block_count: cemented_count,
        })
    }
}
