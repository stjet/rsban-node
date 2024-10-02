use std::sync::Arc;
use rsnano_node::node::Node;
use rsnano_rpc_messages::SuccessDto;
use serde_json::to_string_pretty;

pub async fn unchecked_clear(node: Arc<Node>) -> String {
    node.unchecked.clear();
    to_string_pretty(&SuccessDto::new()).unwrap()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_core::{HashOrAccount, UncheckedInfo};
    use test_helpers::{send_block, System};

    #[test]
    fn unchecked_clear() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        //let block = send_block(node.clone());

        //node.unchecked.put(HashOrAccount::zero(), UncheckedInfo::new(Arc::new(block)));

        //assert!(!node.unchecked.is_empty());

        node.tokio.block_on(async {
            rpc_client
                .unchecked_clear()
                .await
                .unwrap()
        });

        assert!(node.unchecked.is_empty());
    
        server.abort();
    }
}