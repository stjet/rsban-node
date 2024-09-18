use std::{collections::HashMap, sync::Arc};
use rsnano_core::Account;
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
    use rsnano_core::{Amount, KeyPair, WalletId, DEV_GENESIS_KEY};

    #[test]
    fn representatives_online() {
        let mut system = System::new();
        let node = system.make_node();
        let node2 = system.make_node(); // Create node2
        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let wallet = WalletId::zero();
        let wallet2 = WalletId::random();
        node.wallets.create(wallet);
        node.wallets.create(wallet2);
        node.wallets.insert_adhoc2(&wallet, &(*DEV_GENESIS_KEY).private_key(), true).unwrap();
        let key = KeyPair::new();
        let private_key = key.private_key();
        let new_rep = key.public_key();
        //let private_key = RawKey::zero();
        //let new_rep: PublicKey = PublicKey::try_from(&private_key).unwrap();
        node.wallets.insert_adhoc2(&wallet2, &private_key, true).unwrap();
        //let new_rep = node.wallets.deterministic_insert2(&wallet2, true).unwrap();
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

        // Add a new representative
        let new_rep = node.wallets.deterministic_insert2(&wallet2, true).unwrap();
        let send_to_new_rep = node.wallets.send_action2(&wallet, *DEV_GENESIS_ACCOUNT, new_rep.into(), node.config.receive_minimum, 0, true, None).unwrap();
        node.process_active(send_to_new_rep.clone());

        // Ensure the new representative receives the funds
        assert_timely_msg(
            Duration::from_secs(5),
            || node.ledger.any().get_block(&node.ledger.read_txn(), &send_to_new_rep.hash()).is_some(),
            "send to new rep not received",
        );

        // TODO: this fails
        //let receive = node.wallets.receive_action2(&wallet2, send_to_new_rep.hash(), new_rep.into(), node.config.receive_minimum, send.destination().unwrap(), 0, true).unwrap().unwrap();
        //node.process_active(receive.clone());

        // Change representative for genesis account
        let change = node.wallets.change_action2(&wallet, *DEV_GENESIS_ACCOUNT, new_rep.into(), 0, true).unwrap().unwrap();
        node.process_active(change.clone());

        // Ensure we have two online representatives
        assert_timely_msg(
            Duration::from_secs(10),
            || node.online_reps.lock().unwrap().online_reps().count() == 2
                && node2.online_reps.lock().unwrap().online_reps().count() == 2,
            "two representatives not online on both nodes",
        );

        // Test filtering by accounts
        let filtered_result = node.tokio.block_on(async {
            rpc_client.representatives_online(Some(true), Some(vec![new_rep.into()])).await.unwrap()
        });

        assert_eq!(filtered_result.value.len(), 1);
        assert!(filtered_result.value.contains_key(&new_rep.into()));
        assert!(!filtered_result.value.contains_key(&(*DEV_GENESIS_ACCOUNT)));

        // Ensure node2 has the same view of online representatives
        let node2_online_reps = node2.online_reps.lock().unwrap().online_reps().count();
        assert_eq!(node2_online_reps, 2, "Node2 doesn't have the correct number of online representatives");

        server.abort();
    }
}

