use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{ErrorDto, RpcDto, WalletRepresentativeDto, WalletRpcMessage};

impl RpcCommandHandler {
    pub(crate) fn wallet_representative(&self, args: WalletRpcMessage) -> RpcDto {
        match self.node.wallets.get_representative(args.wallet) {
            Ok(representative) => {
                RpcDto::WalletRepresentative(WalletRepresentativeDto::new(representative.into()))
            }
            Err(e) => RpcDto::Error(ErrorDto::WalletsError(e)),
        }
    }
}
