use crate::command_handler::RpcCommandHandler;
use rsnano_node::wallets::WalletsExt;
use rsnano_rpc_messages::{BlockDto, SendArgs};

impl RpcCommandHandler {
    pub(crate) fn send(&self, args: SendArgs) -> anyhow::Result<BlockDto> {
        self.ensure_control_enabled()?;
        let block_hash =
            self.node
                .wallets
                .send_sync(args.wallet, args.source, args.destination, args.amount);
        Ok(BlockDto::new(block_hash))
    }
}
