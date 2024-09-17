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

    let mut representatives_with_weight = HashMap::new();
    let mut representatives_without_weight = HashMap::new();

    let accounts_to_filter = accounts.unwrap_or_default();
    let filtering = !accounts_to_filter.is_empty();

    for pk in online_reps {
        let account = Account::from(pk.clone());

        if filtering {
            if !accounts_to_filter.contains(&account) {
                continue;
            }
        }

        if weight {
            let account_weight = node.ledger.weight(&pk);
            representatives_with_weight.insert(account, account_weight);
        } else {
            representatives_without_weight.insert(account, String::new());
        }
    }

    let dto = if weight {
        RepresentativesOnlineDto::WithWeight(representatives_with_weight)
    } else {
        RepresentativesOnlineDto::WithoutWeight(representatives_without_weight)
    };

    to_string_pretty(&dto).unwrap()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_PUB_KEY};
    use rsnano_node::wallets::{Wallet, WalletsExt};
    use rsnano_rpc_client::NanoRpcClient;
    use test_helpers::{assert_timely_msg, System};
    use rsnano_core::{Amount, PublicKey, WalletId, DEV_GENESIS_KEY};

    #[test]
    fn representatives_online() {
        let mut system = System::new();
        let node = system.make_node();
        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let result = node.tokio.block_on(async {
            rpc_client.representatives_online(None, None).await.unwrap()
        });

        assert_eq!(result.value.len(), 1);
        assert!(result.value.contains_key(&DEV_GENESIS_ACCOUNT));

        server.abort();
    }

    #[test]
    fn representatives_online_with_weight() {
        let mut system = System::new();
        let node = system.make_node();
        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let result = node.tokio.block_on(async {
            rpc_client.representatives_online(Some(true), None).await.unwrap()
        });

        assert_eq!(result.value.len(), 1);
        let (account, weight) = result.value.iter().next().unwrap();
        assert_eq!(*account, *DEV_GENESIS_ACCOUNT);
        assert_eq!(*weight, Amount::MAX);

        server.abort();
    }

    #[test]
    fn representatives_online_with_accounts_filter() {
        let mut system = System::new();
        let node = system.make_node();
        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let wallet = WalletId::zero();
        let wallet2 = WalletId::random();
        node.wallets.create(wallet);
        node.wallets.create(wallet2);
        node.wallets.insert_adhoc2(&wallet, &(*DEV_GENESIS_KEY).private_key(), true).unwrap();
        let new_rep = node.wallets.deterministic_insert2(&wallet2, true).unwrap();
        let send_amount = Amount::from(1000000);
        
        let send = node.wallets.send_action2(&wallet, *DEV_GENESIS_ACCOUNT, new_rep.into(), send_amount, 0, true, None).unwrap();
        let send_hash = send.hash();
        node.process_active(send.clone());

        assert_timely_msg(
            Duration::from_secs(5),
            || node.active.active(&send),
            "open not active on node 1",
        );

        //let wallets = node.wallets.mutex.lock().unwrap();
        //let wallet = wallets.get(&wallet).unwrap();

        let receive = node.wallets.receive_action2(&wallet2, send_hash, new_rep, send_amount, new_rep.into(), 0, true).unwrap();
        /*node.process_active(receive);

        let change = node.wallets.change_action(&wallet, *DEV_GENESIS_ACCOUNT, new_rep, 0, true).unwrap();
        node.process_active(change);

        // Wait for the new representative to be recognized
        std::thread::sleep(std::time::Duration::from_secs(1));

        let filtered_accounts = vec![Account::from(new_rep)];
        
        let result = node.tokio.block_on(async {
            rpc_client.representatives_online(Some(true), Some(filtered_accounts)).await.unwrap()
        });
            
        assert_eq!(result.value.len(), 1);
        assert!(result.value.contains_key(&Account::from(new_rep)));
        assert_eq!(result.value[&Account::from(new_rep)], send_amount);*/

        server.abort();
    }
}