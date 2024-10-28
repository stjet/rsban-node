use crate::command_handler::RpcCommandHandler;
use rsnano_node::wallets::WalletsExt;
use rsnano_rpc_messages::{ExistsDto, WalletRpcMessage};

impl RpcCommandHandler {
    pub(crate) fn search_receivable(&self, args: WalletRpcMessage) -> anyhow::Result<ExistsDto> {
        self.node.wallets.search_receivable_wallet(args.wallet)?;
        Ok(ExistsDto::new(true))
    }
}
