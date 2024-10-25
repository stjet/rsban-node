use crate::command_handler::RpcCommandHandler;
use rsnano_core::PublicKey;
use rsnano_rpc_messages::{AccountMoveArgs, ErrorDto, MovedDto, RpcDto};

impl RpcCommandHandler {
    pub(crate) fn account_move(&self, args: AccountMoveArgs) -> RpcDto {
        if self.enable_control {
            let public_keys: Vec<PublicKey> =
                args.accounts.iter().map(|account| account.into()).collect();
            let result = self
                .node
                .wallets
                .move_accounts(&args.source, &args.wallet, &public_keys);

            match result {
                Ok(()) => RpcDto::Moved(MovedDto::new(true)),
                Err(e) => RpcDto::Error(ErrorDto::WalletsError(e)),
            }
        } else {
            RpcDto::Error(ErrorDto::RPCControlDisabled)
        }
    }
}
