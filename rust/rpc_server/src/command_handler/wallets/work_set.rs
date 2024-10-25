use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{ErrorDto, RpcDto, SuccessDto, WorkSetArgs};

impl RpcCommandHandler {
    pub(crate) fn work_set(&self, args: WorkSetArgs) -> RpcDto {
        if self.enable_control {
            match self
                .node
                .wallets
                .work_set(&args.wallet, &args.account.into(), args.work.into())
            {
                Ok(_) => RpcDto::WorkSet(SuccessDto::new()),
                Err(e) => RpcDto::Error(ErrorDto::WalletsError(e)),
            }
        } else {
            RpcDto::Error(ErrorDto::RPCControlDisabled)
        }
    }
}
