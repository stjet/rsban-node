use std::{collections::HashMap, sync::Arc};
use rsnano_core::{Account, Amount};
use rsnano_node::node::Node;
use rsnano_rpc_messages::AccountsWithAmountsDto;
use serde_json::to_string_pretty;

pub async fn representatives_online(node: Arc<Node>) -> String {
    let representatives: HashMap<Account, Amount> = node.online_reps.lock().unwrap()
        .online_reps()
        .map(|pk| (Account::from(pk.clone()), Amount::zero()))
        .collect();
    to_string_pretty(&AccountsWithAmountsDto::new("representatives".to_string(), representatives)).unwrap()
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use test_helpers::System;

    #[test]
    fn representatives_online_rpc_response() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let result = node.tokio.block_on(async {
            rpc_client
                .representatives_online()
                .await
                .unwrap()
        });

        server.abort();
    }
}