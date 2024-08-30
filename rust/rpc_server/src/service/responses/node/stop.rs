use crate::service::responses::format_success_message;
use rsnano_node::node::{Node, NodeExt};
use std::sync::Arc;

pub async fn stop(node: Arc<Node>) -> String {
    node.stop();
    format_success_message()
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use test_helpers::System;

    #[test]
    fn stop() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        node.tokio
            .block_on(async { rpc_client.stop().await.unwrap() });

        assert!(node.is_stopped());

        server.abort();
    }
}
