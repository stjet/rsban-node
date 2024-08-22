use anyhow::Result;
use rsnano_core::{Account, Amount};
use rsnano_node::node::Node;
use rsnano_rpc_messages::AccountBalanceResponse;
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn account_balance(
    node: Arc<Node>,
    account: Account,
    only_confirmed: Option<bool>,
) -> String {
    let tx = node.ledger.read_txn();
    let only_confirmed = only_confirmed.unwrap_or(true);

    let balance = if only_confirmed {
        node.ledger
            .confirmed()
            .account_balance(&tx, &account)
            .unwrap_or(Amount::zero())
    } else {
        node.ledger
            .any()
            .account_balance(&tx, &account)
            .unwrap_or(Amount::zero())
    };

    let pending = node
        .ledger
        .account_receivable(&tx, &account, only_confirmed);

    let account_balance = AccountBalanceResponse::new(balance, pending, pending);

    to_string_pretty(&account_balance).unwrap()
}

#[cfg(test)]
mod tests {
    use crate::{run_rpc_server, RpcServerConfig};
    use anyhow::Result;
    use reqwest::Url;
    use rsnano_core::{Amount, BlockEnum, StateBlock, DEV_GENESIS_KEY};
    use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
    use rsnano_node::node::Node;
    use rsnano_rpc_client::NanoRpcClient;
    use std::net::{IpAddr, SocketAddr};
    use std::str::FromStr;
    use std::sync::Arc;
    use std::time::Duration;
    use test_helpers::{assert_timely_msg, get_available_port, System};

    fn setup_node_and_rpc_server() -> (
        Arc<Node>,
        Arc<NanoRpcClient>,
        tokio::task::JoinHandle<Result<(), anyhow::Error>>,
    ) {
        let mut system = System::new();
        let node = system.make_node();

        let send1 = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            *DEV_GENESIS_HASH,
            *DEV_GENESIS_PUB_KEY,
            Amount::MAX - Amount::raw(1),
            DEV_GENESIS_KEY.account().into(),
            &DEV_GENESIS_KEY,
            node.work_generate_dev((*DEV_GENESIS_HASH).into()),
        ));

        node.process_active(send1.clone());
        assert_timely_msg(
            Duration::from_secs(5),
            || node.active.active(&send1),
            "not active on node 1",
        );

        let rpc_server_config = RpcServerConfig::default();
        let ip_addr = IpAddr::from_str(&rpc_server_config.address).unwrap();
        let port = get_available_port();
        let socket_addr = SocketAddr::new(ip_addr, port);

        let server = node.clone().async_rt.tokio.spawn(run_rpc_server(
            node.clone(),
            socket_addr,
            rpc_server_config.enable_control,
        ));

        let rpc_url = format!("http://[::1]:{}/", port);
        let rpc_client = Arc::new(NanoRpcClient::new(Url::parse(&rpc_url).unwrap()));

        (node, rpc_client, server)
    }

    #[test]
    fn account_balance_only_confirmed_none() {
        let (node, rpc_client, server) = setup_node_and_rpc_server();

        let result = node.async_rt.tokio.block_on(async {
            rpc_client
                .account_balance(DEV_GENESIS_KEY.public_key().as_account(), None)
                .await
                .unwrap()
        });

        assert_eq!(
            result.balance,
            Amount::raw(340282366920938463463374607431768211455)
        );

        assert_eq!(result.pending, Amount::zero());

        assert_eq!(result.receivable, Amount::zero());

        server.abort();
    }

    #[test]
    fn account_balance_only_confirmed_true() {
        let (node, rpc_client, server) = setup_node_and_rpc_server();

        let result = node.async_rt.tokio.block_on(async {
            rpc_client
                .account_balance(DEV_GENESIS_KEY.public_key().as_account(), Some(true))
                .await
                .unwrap()
        });

        assert_eq!(
            result.balance,
            Amount::raw(340282366920938463463374607431768211455)
        );

        assert_eq!(result.pending, Amount::zero());

        assert_eq!(result.receivable, Amount::zero());

        server.abort();
    }

    #[test]
    fn account_balance_only_confirmed_false() {
        let (node, rpc_client, server) = setup_node_and_rpc_server();

        let result = node.async_rt.tokio.block_on(async {
            rpc_client
                .account_balance(DEV_GENESIS_KEY.public_key().as_account(), Some(false))
                .await
                .unwrap()
        });

        assert_eq!(
            result.balance,
            Amount::raw(340282366920938463463374607431768211454)
        );

        assert_eq!(result.pending, Amount::raw(1));

        assert_eq!(result.receivable, Amount::raw(1));

        server.abort();
    }
}
