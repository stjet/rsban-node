use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{ErrorDto, FrontiersDto, RpcDto, WalletRpcMessage};
use std::collections::HashMap;

impl RpcCommandHandler {
    pub(crate) fn wallet_frontiers(&self, args: WalletRpcMessage) -> RpcDto {
        let tx = self.node.ledger.read_txn();
        let mut frontiers = HashMap::new();

        let accounts = match self.node.wallets.get_accounts_of_wallet(&args.wallet) {
            Ok(accounts) => accounts,
            Err(e) => return RpcDto::Error(ErrorDto::WalletsError(e)),
        };

        for account in accounts {
            if let Some(block_hash) = self.node.ledger.any().account_head(&tx, &account) {
                frontiers.insert(account, block_hash);
            }
        }
        RpcDto::WalletFrontiers(FrontiersDto::new(frontiers))
    }
}
