use std::{collections::HashMap, sync::Arc};
use rsnano_core::{Account, Amount};
use rsnano_node::node::Node;
use rsnano_rpc_messages::RepresentativesOnlineDto;
use serde_json::to_string_pretty;

pub async fn representatives_online(
    node: Arc<Node>,
    weight: Option<bool>,
    accounts: Option<Vec<Account>>
) -> String {
    let lock = node.online_reps.lock().unwrap();
    let online_reps = lock.online_reps();
    let mut representatives: HashMap<Account, Amount> = HashMap::new();

    for pk in online_reps {
        if let Some(ref filter_accounts) = accounts {
            if !filter_accounts.contains(&Account::from(pk.clone())) {
                continue;
            }
        }

        let account = Account::from(pk.clone());
        let account_weight = node.ledger.weight(&pk);
        representatives.insert(account, account_weight);
    }

    let dto = if weight.unwrap_or(false) {
        RepresentativesOnlineDto::new_with_weight(representatives)
    } else {
        RepresentativesOnlineDto::new_without_weight(representatives.keys().cloned().collect())
    };

    to_string_pretty(&dto).unwrap()
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use test_helpers::System;

    #[test]
    fn representatives_online() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let result = node.tokio.block_on(async {
            rpc_client
                .representatives_online(None, None)
                .await
                .unwrap()
        });

        server.abort();
    }
}