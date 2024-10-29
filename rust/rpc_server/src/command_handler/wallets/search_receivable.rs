use crate::command_handler::RpcCommandHandler;
use rsnano_node::wallets::{WalletsError, WalletsExt};
use rsnano_rpc_messages::{StartedDto, WalletRpcMessage};

impl RpcCommandHandler {
    pub(crate) fn search_receivable(&self, args: WalletRpcMessage) -> anyhow::Result<StartedDto> {
        match self.node.wallets.search_receivable_wallet(args.wallet) {
            Ok(_) => Ok(StartedDto::new(true)),
            Err(WalletsError::WalletLocked) => Ok(StartedDto::new(false)),
            Err(e) => Err(e.into()),
        }
    }
}
