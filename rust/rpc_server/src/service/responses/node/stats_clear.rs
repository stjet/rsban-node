use std::sync::Arc;
use rsnano_node::node::Node;
use rsnano_rpc_messages::SuccessDto;
use serde_json::to_string_pretty;

pub async fn stats_clear(node: Arc<Node>) -> String {
    node.stats.clear();
    to_string_pretty(&SuccessDto::new()).unwrap()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_node::stats::{DetailType, Direction, StatType};
    use test_helpers::System;

    #[test]
    fn stats_clear() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        node.tokio.block_on(async {
            rpc_client
                .stats_clear()
                .await
                .unwrap()
        });

        assert_eq!(
            node.stats.count(StatType::Ledger, DetailType::Fork, Direction::In),
            0
        );
    
        assert!(node.stats.last_reset() <= Duration::from_secs(5));

        server.abort();
    }
}