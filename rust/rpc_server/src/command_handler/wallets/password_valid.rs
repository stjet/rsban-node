use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{ValidDto, WalletRpcMessage};

impl RpcCommandHandler {
    pub(crate) fn password_valid(&self, args: WalletRpcMessage) -> anyhow::Result<ValidDto> {
        let valid = self.node.wallets.valid_password(&args.wallet)?;
        Ok(ValidDto::new(valid))
    }
}
