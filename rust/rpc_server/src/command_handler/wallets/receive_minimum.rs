use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::AmountRpcMessage;

impl RpcCommandHandler {
    pub(crate) fn receive_minimum(&self) -> anyhow::Result<AmountRpcMessage> {
        self.ensure_control_enabled()?;
        let amount = self.node.config.receive_minimum;
        Ok(AmountRpcMessage::new(amount))
    }
}
