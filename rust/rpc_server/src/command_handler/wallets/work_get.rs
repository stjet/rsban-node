use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{ErrorDto, RpcDto, WalletWithAccountArgs, WorkDto};

impl RpcCommandHandler {
    pub(crate) fn work_get(&self, args: WalletWithAccountArgs) -> RpcDto {
        if self.enable_control {
            match self
                .node
                .wallets
                .work_get2(&args.wallet, &args.account.into())
            {
                Ok(work) => RpcDto::WorkGet(WorkDto::new(work.into())),
                Err(e) => RpcDto::Error(ErrorDto::WalletsError(e)),
            }
        } else {
            RpcDto::Error(ErrorDto::RPCControlDisabled)
        }
    }
}
