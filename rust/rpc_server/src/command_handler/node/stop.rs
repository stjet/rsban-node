use crate::command_handler::RpcCommandHandler;
use rsnano_node::NodeExt;
use rsnano_rpc_messages::SuccessDto;

impl RpcCommandHandler {
    pub(crate) fn stop(&self) -> anyhow::Result<SuccessDto> {
        self.ensure_control_enabled()?;
        self.node.stop();
        Ok(SuccessDto::new())
    }
}
