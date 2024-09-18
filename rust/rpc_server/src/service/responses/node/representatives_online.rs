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
    let weight = weight.unwrap_or(false);

    let mut representatives = HashMap::new();

    let accounts_to_filter = accounts.unwrap_or_default();
    let filtering = !accounts_to_filter.is_empty();

    for pk in online_reps {
        let account = Account::from(pk.clone());

        if filtering && !accounts_to_filter.contains(&account) {
            continue;
        }

        let account_weight = if weight {
            Some(node.ledger.weight(&pk))
        } else {
            None
        };

        representatives.insert(account, account_weight);
    }

    let dto = RepresentativesOnlineDto { representatives };

    to_string_pretty(&dto).unwrap()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_ledger::DEV_GENESIS_ACCOUNT;
    use rsnano_node::wallets::WalletsExt;
    use test_helpers::{assert_timely_msg, System};
    use rsnano_core::{Amount, WalletId, DEV_GENESIS_KEY};

    #[test]
    fn representatives_online() {
        let mut system = System::new();
        let node = system.make_node();
        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let wallet = WalletId::zero();
        let wallet2 = WalletId::random();
        node.wallets.create(wallet);
        node.wallets.create(wallet2);
        node.wallets.insert_adhoc2(&wallet, &(*DEV_GENESIS_KEY).private_key(), true).unwrap();
        let new_rep = node.wallets.deterministic_insert2(&wallet2, true).unwrap();
        let send_amount = Amount::raw(1_000_000_000_000_000_000_000_000u128); // 1 Gxrb

        // Send funds to new representative
        let send = node.wallets.send_action2(&wallet, *DEV_GENESIS_ACCOUNT, new_rep.into(), send_amount, 0, true, None).unwrap();
        node.process_active(send.clone());

        assert_timely_msg(
            Duration::from_secs(10),
            || node.online_reps.lock().unwrap().online_reps().next().is_some(),
            "representatives not online",
        );

        // Check online weight
        let online_weight = node.online_reps.lock().unwrap().online_weight();
        assert_eq!(online_weight, Amount::MAX - send_amount);

        // RPC call
        let result = node.tokio.block_on(async {
            rpc_client.representatives_online(Some(false), None).await.unwrap()
        });

        // Check if genesis account is in the representatives list
        assert!(result.value.contains_key(&(*DEV_GENESIS_ACCOUNT)));

        // Ensure weight is not included
        assert!(result.value.values().all(|v| v.is_none()));

        // Check with weight
        let result_with_weight = node.tokio.block_on(async {
            rpc_client.representatives_online(Some(true), None).await.unwrap()
        });

        // Check if genesis account is in the representatives list and has the correct weight
        let genesis_weight = result_with_weight.value.get(&(DEV_GENESIS_ACCOUNT)).unwrap().unwrap();
        assert_eq!(genesis_weight, (Amount::MAX - send_amount));

        // Ensure the block is received
        assert_timely_msg(
            Duration::from_secs(5),
            || node.ledger.any().get_block(&node.ledger.read_txn(), &send.hash()).is_some(),
            "send block not received",
        );

        server.abort();
    }
}

