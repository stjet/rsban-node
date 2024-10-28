use crate::command_handler::RpcCommandHandler;
use rsnano_node::wallets::WalletsExt;
use rsnano_rpc_messages::{WalletChangeSeedArgs, WalletChangeSeedDto};

impl RpcCommandHandler {
    pub(crate) fn wallet_change_seed(&self, args: WalletChangeSeedArgs) -> WalletChangeSeedDto {
        let (restored_count, last_restored_account) = self
            .node
            .wallets
            .change_seed(args.wallet, &args.seed, args.count.unwrap_or(0))
            .unwrap();
        WalletChangeSeedDto::new(last_restored_account, restored_count)
    }
}
