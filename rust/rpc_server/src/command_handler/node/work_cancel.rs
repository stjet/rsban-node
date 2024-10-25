use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{ErrorDto, HashRpcMessage, RpcDto, SuccessDto};

impl RpcCommandHandler {
    pub(crate) fn work_cancel(&self, args: HashRpcMessage) -> RpcDto {
        if self.enable_control {
            self.node.distributed_work.cancel(args.hash.into());
            RpcDto::WorkCancel(SuccessDto::new())
        } else {
            RpcDto::Error(ErrorDto::RPCControlDisabled)
        }
    }
}
