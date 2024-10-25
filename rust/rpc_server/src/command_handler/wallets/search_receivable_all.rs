use crate::command_handler::RpcCommandHandler;
use rsnano_node::NodeExt;
use rsnano_rpc_messages::SuccessDto;

impl RpcCommandHandler {
    pub(crate) fn search_receivable_all(&self) -> anyhow::Result<SuccessDto> {
        self.ensure_control_enabled()?;
        self.node.search_receivable_all();
        Ok(SuccessDto::new())
    }
}
