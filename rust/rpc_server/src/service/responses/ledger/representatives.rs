use std::{collections::HashMap, sync::Arc};
use rsnano_core::{Account, Amount};
use rsnano_node::node::Node;
use rsnano_rpc_messages::AccountsWithAmountsDto;
use serde_json::to_string_pretty;

pub async fn representatives(node: Arc<Node>) -> String {
    let representatives: HashMap<Account, Amount> = node.ledger.rep_weights.read()
        .iter()
        .map(|(pk, amount)| (Account::from(pk), *amount))
        .collect();
    to_string_pretty(&AccountsWithAmountsDto::new("representatives".to_string(), representatives)).unwrap()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_core::Amount;
    use rsnano_ledger::DEV_GENESIS_ACCOUNT;
    use test_helpers::System;

    #[test]
    fn representatives_rpc_response() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let result = node.tokio.block_on(async {
            rpc_client
                .representatives()
                .await
                .unwrap()
        });

        let mut representatives = HashMap::new();
        representatives.insert(*DEV_GENESIS_ACCOUNT, Amount::MAX);

        assert_eq!(result.value, representatives);

        server.abort();
    }
}