use rsnano_node::node::{Node, NodeExt};
use rsnano_rpc_messages::{ErrorDto, SuccessDto};
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn search_receivable_all(node: Arc<Node>, enable_control: bool) -> String {
    if enable_control {
        node.search_receivable_all();
        to_string_pretty(&SuccessDto::new()).unwrap()
    } else {
        to_string_pretty(&ErrorDto::new("RPC control is disabled".to_string())).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use rsnano_core::{Account, Amount, BlockEnum, Link, Root, StateBlock, WalletId, DEV_GENESIS_KEY};
    use rsnano_ledger::{BlockStatus, DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
    use rsnano_node::{node::Node, wallets::WalletsExt};
    use std::{sync::Arc, time::Duration};
    use test_helpers::{assert_timely_eq, send_block_to, setup_rpc_client_and_server, System};

    #[test]
    fn search_receivable_all() {
        let mut system = System::new();
        let node: Arc<Node> = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let wallet_id = WalletId::zero();
        node.wallets.create(wallet_id);
        node.wallets
            .insert_adhoc2(&wallet_id, &DEV_GENESIS_KEY.private_key(), false)
            .unwrap();

            let send = BlockEnum::State(StateBlock::new(
                *DEV_GENESIS_ACCOUNT,
                *DEV_GENESIS_HASH,
                *DEV_GENESIS_PUB_KEY,
                Amount::MAX - node.config.receive_minimum,
                Link::from(*DEV_GENESIS_ACCOUNT),
                &DEV_GENESIS_KEY,
                node.work_generate_dev(Root::from(*DEV_GENESIS_HASH)),
            ));

        assert_eq!(node.process_local(send).unwrap(), BlockStatus::Progress);

        node.tokio.block_on(async {
            rpc_client.search_receivable_all().await.unwrap();
        });

        assert_timely_eq(
            Duration::from_secs(10),
            || node.balance(&DEV_GENESIS_KEY.account()),
            Amount::MAX,
        );

        server.abort();
    }

    #[test]
    fn search_receivable_all_fails_without_enable_control() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let result = node
            .tokio
            .block_on(async { rpc_client.search_receivable_all().await });

        assert_eq!(
            result.err().map(|e| e.to_string()),
            Some("node returned error: \"RPC control is disabled\"".to_string())
        );

        server.abort();
    }
}
