use crate::command_handler::RpcCommandHandler;
use rsnano_node::wallets::WalletsExt;
use rsnano_rpc_messages::{BlockDto, ErrorDto, RpcDto, SendArgs};

impl RpcCommandHandler {
    pub(crate) fn send(&self, args: SendArgs) -> RpcDto {
        if self.enable_control {
            let block_hash = self.node.wallets.send_sync(
                args.wallet,
                args.source,
                args.destination,
                args.amount,
            );
            RpcDto::Send(BlockDto::new(block_hash))
        } else {
            RpcDto::Error(ErrorDto::RPCControlDisabled)
        }
    }
}
