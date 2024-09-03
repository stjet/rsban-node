use rsnano_core::Account;
use rsnano_node::node::Node;
use rsnano_rpc_messages::FrontiersDto;
use serde_json::to_string_pretty;
use std::{collections::HashMap, sync::Arc};

pub async fn accounts_frontiers(node: Arc<Node>, accounts: Vec<Account>) -> String {
    let tx = node.ledger.read_txn();
    let mut frontiers = HashMap::new();

    for account in accounts {
        if let Some(block_hash) = node.ledger.any().account_head(&tx, &account) {
            frontiers.insert(account, block_hash);
        }
    }
    to_string_pretty(&FrontiersDto::new(frontiers)).unwrap()
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH};
    use test_helpers::System;

    #[test]
    fn accounts_frontiers() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let result = node.tokio.block_on(async {
            rpc_client
                .accounts_frontiers(vec![*DEV_GENESIS_ACCOUNT])
                .await
                .unwrap()
        });

        assert_eq!(
            result.frontiers.get(&*DEV_GENESIS_ACCOUNT).unwrap(),
            &*DEV_GENESIS_HASH
        );

        server.abort();
    }
}
