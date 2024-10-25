use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{HashRpcMessage, SuccessDto};

impl RpcCommandHandler {
    pub(crate) fn work_cancel(&self, args: HashRpcMessage) -> anyhow::Result<SuccessDto> {
        self.ensure_control_enabled()?;
        self.node.distributed_work.cancel(args.hash.into());
        Ok(SuccessDto::new())
    }
}
