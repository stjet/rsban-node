use rsnano_node::Node;
use rsnano_rpc_messages::CountDto;
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn frontier_count(node: Arc<Node>) -> String {
    to_string_pretty(&CountDto::new(
        node.ledger.account_count(),
    ))
    .unwrap()
}
