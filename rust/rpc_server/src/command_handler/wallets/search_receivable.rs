use crate::command_handler::RpcCommandHandler;
use rsnano_node::wallets::WalletsExt;
use rsnano_rpc_messages::{ErrorDto, ExistsDto, RpcDto, WalletRpcMessage};

impl RpcCommandHandler {
    pub(crate) fn search_receivable(&self, args: WalletRpcMessage) -> RpcDto {
        if self.enable_control {
            match self.node.wallets.search_receivable_wallet(args.wallet) {
                Ok(_) => RpcDto::SearchReceivable(ExistsDto::new(true)),
                Err(e) => RpcDto::Error(ErrorDto::WalletsError(e)),
            }
        } else {
            RpcDto::Error(ErrorDto::RPCControlDisabled)
        }
    }
}
