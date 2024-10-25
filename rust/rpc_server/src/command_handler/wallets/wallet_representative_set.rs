use crate::command_handler::RpcCommandHandler;
use rsnano_node::wallets::WalletsExt;
use rsnano_rpc_messages::{ErrorDto, RpcDto, SetDto, WalletRepresentativeSetArgs};

impl RpcCommandHandler {
    pub(crate) fn wallet_representative_set(&self, args: WalletRepresentativeSetArgs) -> RpcDto {
        if self.enable_control {
            let update_existing = args.update_existing_accounts.unwrap_or(false);
            match self.node.wallets.set_representative(
                args.wallet,
                args.account.into(),
                update_existing,
            ) {
                Ok(_) => RpcDto::WalletRepresentativeSet(SetDto::new(true)),
                Err(e) => RpcDto::Error(ErrorDto::WalletsError(e)),
            }
        } else {
            RpcDto::Error(ErrorDto::RPCControlDisabled)
        }
    }
}
