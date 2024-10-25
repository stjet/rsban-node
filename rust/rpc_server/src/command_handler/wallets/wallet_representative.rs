use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{WalletRepresentativeDto, WalletRpcMessage};

impl RpcCommandHandler {
    pub(crate) fn wallet_representative(
        &self,
        args: WalletRpcMessage,
    ) -> anyhow::Result<WalletRepresentativeDto> {
        let representative = self.node.wallets.get_representative(args.wallet)?;
        Ok(WalletRepresentativeDto::new(representative.into()))
    }
}
