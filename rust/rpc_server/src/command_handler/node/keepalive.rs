use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{HostWithPortArgs, StartedResponse};

impl RpcCommandHandler {
    pub(crate) fn keepalive(&self, args: HostWithPortArgs) -> anyhow::Result<StartedResponse> {
        self.node
            .rep_crawler
            .keepalive_or_connect(args.address, args.port.into());
        Ok(StartedResponse::new(true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_node::Node;
    use rsnano_rpc_messages::RpcCommand;
    use std::sync::Arc;

    #[tokio::test]
    #[ignore = "wip"]
    async fn keepalive() {
        let node = Arc::new(Node::new_null());
        let tracker = node.rep_crawler.track_keepalives();
        let (tx_stop, _rx_stop) = tokio::sync::oneshot::channel();
        let cmd_handler = RpcCommandHandler::new(node, true, tx_stop);

        let result = cmd_handler.handle(RpcCommand::keepalive("foobar.com", 123));
        // TODO check result
    }
}
