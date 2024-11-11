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
    use rsnano_core::utils::Peer;
    use rsnano_node::Node;
    use rsnano_rpc_messages::{RpcCommand, RpcError};
    use std::sync::Arc;

    #[tokio::test]
    async fn keepalive() {
        let node = Arc::new(Node::new_null());
        let keepalive_tracker = node.rep_crawler.track_keepalives();
        let (tx_stop, _rx_stop) = tokio::sync::oneshot::channel();
        let cmd_handler = RpcCommandHandler::new(node, true, tx_stop);

        let result = cmd_handler.handle(RpcCommand::keepalive("foobar.com", 123));

        assert_eq!(
            result,
            serde_json::to_value(StartedResponse::new(true)).unwrap()
        );

        let keepalives = keepalive_tracker.output();
        assert_eq!(keepalives, [Peer::new("foobar.com", 123)]);
    }

    #[tokio::test]
    async fn keepalive_fails_without_rpc_control_enabled() {
        let node = Arc::new(Node::new_null());
        let (tx_stop, _rx_stop) = tokio::sync::oneshot::channel();
        let cmd_handler = RpcCommandHandler::new(node, false, tx_stop);

        let result = cmd_handler.handle(RpcCommand::keepalive("foobar.com", 123));

        assert_eq!(
            result,
            serde_json::to_value(RpcError::new("RPC control is disabled")).unwrap()
        );
    }
}
