use crate::server::service::format_error_message;
use rsnano_core::Account;
use rsnano_node::node::Node;
use serde::Serialize;
use serde_json::to_string_pretty;
use std::sync::Arc;

#[derive(Serialize)]
struct AccountBalance {
    balance: String,
    pending: String,
    receivable: String,
}

impl AccountBalance {
    fn new(balance: String, pending: String, receivable: String) -> Self {
        Self {
            balance,
            pending,
            receivable,
        }
    }
}

pub(crate) async fn account_balance(
    node: Arc<Node>,
    account_str: String,
    only_confirmed: Option<String>,
) -> String {
    let tx = node.ledger.read_txn();

    let account = match Account::decode_account(&account_str) {
        Ok(account) => account,
        Err(_) => return format_error_message("Bad account number"),
    };

    let balance = match node.ledger.confirmed().account_balance(&tx, &account) {
        Some(balance) => balance,
        None => return format_error_message("Account not found"),
    };

    let only_confirmed = match only_confirmed.as_deref() {
        Some("true") => true,
        Some("false") => false,
        _ => true,
    };

    let pending = node
        .ledger
        .account_receivable(&tx, &account, only_confirmed);

    let account_balance = AccountBalance::new(
        balance.number().to_string(),
        pending.number().to_string(),
        pending.number().to_string(),
    );

    to_string_pretty(&account_balance).unwrap()
}

#[cfg(test)]
mod tests {
    use crate::{run_rpc_server, RpcConfig};
    use anyhow::Result;
    use reqwest::Url;
    use rsnano_core::{Amount, BlockEnum, StateBlock, DEV_GENESIS_KEY};
    use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH};
    use rsnano_node::node::Node;
    use std::{
        net::{IpAddr, Ipv4Addr, SocketAddr},
        str::FromStr,
        sync::Arc,
        time::Duration,
    };
    use test_helpers::{assert_timely_msg, RpcClient, System};
    use tokio::{net::TcpListener, sync::oneshot};
    use tokio_util::sync::CancellationToken;

    #[tokio::test]
    async fn account_balance_test_with_true_include_pending() -> Result<()> {
        let mut system = System::new();
        let node = system.make_node();

        let send1 = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            *DEV_GENESIS_HASH,
            *DEV_GENESIS_ACCOUNT,
            Amount::MAX - Amount::raw(1),
            DEV_GENESIS_KEY.public_key().into(),
            &DEV_GENESIS_KEY,
            node.work_generate_dev((*DEV_GENESIS_HASH).into()),
        ));

        node.process_active(send1.clone());
        assert_timely_msg(
            Duration::from_secs(5),
            || node.active.active(&send1),
            "not active on node 1",
        );

        let rpc_config = RpcConfig::default();
        let ip_addr = IpAddr::from_str(&rpc_config.address).unwrap();

        let _server = start(
            node,
            SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                get_available_port().await,
            ),
            false,
        )
        .await;

        let node_url = format!("http://[::1]:{}/", rpc_config.port);
        let node_client = Arc::new(RpcClient::new(Url::parse(&node_url).unwrap()));

        let result = node_client
            .account_balance(&DEV_GENESIS_KEY.public_key().encode_account(), Some("true"))
            .await?;

        assert_eq!(
            result.get("balance").unwrap().as_str().unwrap(),
            String::from("340282366920938463463374607431768211455")
        );

        assert_eq!(
            result.get("pending").unwrap().as_str().unwrap(),
            String::from("0")
        );

        Ok(())
    }

    #[tokio::test]
    async fn account_balance_test_without_include_pending() -> Result<()> {
        let mut system = System::new();
        let node = system.make_node();

        let send1 = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            *DEV_GENESIS_HASH,
            *DEV_GENESIS_ACCOUNT,
            Amount::MAX - Amount::raw(1),
            DEV_GENESIS_KEY.public_key().into(),
            &DEV_GENESIS_KEY,
            node.work_generate_dev((*DEV_GENESIS_HASH).into()),
        ));

        node.process_active(send1.clone());
        assert_timely_msg(
            Duration::from_secs(5),
            || node.active.active(&send1),
            "not active on node 1",
        );

        let rpc_config = RpcConfig::default();
        let ip_addr = IpAddr::from_str(&rpc_config.address).unwrap();
        let socket_addr = SocketAddr::new(ip_addr, rpc_config.port);

        let _server = start(
            node,
            SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                get_available_port().await,
            ),
            false,
        )
        .await;

        /*let node_url = format!("http://[::1]:{}/", rpc_config.port);
        let node_client = Arc::new(RpcClient::new(Url::parse(&node_url).unwrap()));

        let result = node_client
            .account_balance(&DEV_GENESIS_KEY.public_key().encode_account(), None)
            .await?;

        assert_eq!(
            result.get("balance").unwrap().as_str().unwrap(),
            String::from("340282366920938463463374607431768211455")
        );*/

        Ok(())
    }

    #[tokio::test]
    async fn account_balance_test_with_invalid_include_pending() -> Result<()> {
        let mut system = System::new();
        let node = system.make_node();

        let send1 = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            *DEV_GENESIS_HASH,
            *DEV_GENESIS_ACCOUNT,
            Amount::MAX - Amount::raw(1),
            DEV_GENESIS_KEY.public_key().into(),
            &DEV_GENESIS_KEY,
            node.work_generate_dev((*DEV_GENESIS_HASH).into()),
        ));

        node.process_active(send1.clone());
        assert_timely_msg(
            Duration::from_secs(5),
            || node.active.active(&send1),
            "not active on node 1",
        );

        let rpc_config = RpcConfig::default();
        let ip_addr = IpAddr::from_str(&rpc_config.address).unwrap();
        let socket_addr = SocketAddr::new(ip_addr, rpc_config.port);

        let _server = start(
            node,
            SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                get_available_port().await,
            ),
            false,
        )
        .await;

        /*let node_url = format!("http://[::1]:{}/", rpc_config.port);
        let node_client = Arc::new(RpcClient::new(Url::parse(&node_url).unwrap()));

        let result = node_client
            .account_balance(
                &DEV_GENESIS_KEY.public_key().encode_account(),
                Some("invalid"),
            )
            .await?;

        assert_eq!(
            result.get("balance").unwrap().as_str().unwrap(),
            String::from("340282366920938463463374607431768211455")
        );

        assert_eq!(
            result.get("pending").unwrap().as_str().unwrap(),
            String::from("0")
        );*/

        Ok(())
    }

    #[tokio::test]
    async fn account_balance_test_with_false_include_pending() -> Result<()> {
        let mut system = System::new();
        let node = system.make_node();

        let send1 = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            *DEV_GENESIS_HASH,
            *DEV_GENESIS_ACCOUNT,
            Amount::MAX - Amount::raw(1),
            DEV_GENESIS_KEY.public_key().into(),
            &DEV_GENESIS_KEY,
            node.work_generate_dev((*DEV_GENESIS_HASH).into()),
        ));

        node.process_active(send1.clone());
        assert_timely_msg(
            Duration::from_secs(5),
            || node.active.active(&send1),
            "not active on node 1",
        );

        let rpc_config = RpcConfig::default();
        let ip_addr = IpAddr::from_str(&rpc_config.address).unwrap();
        let socket_addr = SocketAddr::new(ip_addr, rpc_config.port);

        let _server = start(
            node,
            SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                get_available_port().await,
            ),
            false,
        )
        .await;

        /*let node_url = format!("http://[::1]:{}/", rpc_config.port);
        let node_client = Arc::new(RpcClient::new(Url::parse(&node_url).unwrap()));

        let result = node_client
            .account_balance(
                &DEV_GENESIS_KEY.public_key().encode_account(),
                Some("false"),
            )
            .await?;

        assert_eq!(
            result.get("balance").unwrap().as_str().unwrap(),
            String::from("340282366920938463463374607431768211455")
        );

        assert_eq!(
            result.get("pending").unwrap().as_str().unwrap(),
            String::from("1")
        );*/

        Ok(())
    }

    pub(crate) struct DropGuard {
        cancel_token: CancellationToken,
    }

    impl Drop for DropGuard {
        fn drop(&mut self) {
            self.cancel_token.cancel();
        }
    }

    async fn run_server(
        node: Arc<Node>,
        server_addr: SocketAddr,
        enable_control: bool,
        cancel_token: CancellationToken,
        tx_ready: oneshot::Sender<()>,
    ) {
        //let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        tx_ready.send(()).unwrap();
        tokio::select! {
            _ = run_rpc_server(node, server_addr, enable_control) => { },
            _ = cancel_token.cancelled() => { }
        }
    }

    pub(crate) async fn start(
        node: Arc<Node>,
        server_addr: SocketAddr,
        enable_control: bool,
    ) -> DropGuard {
        let guard = DropGuard {
            cancel_token: CancellationToken::new(),
        };
        let cancel_token = guard.cancel_token.clone();
        let (tx_ready, rx_ready) = oneshot::channel::<()>();

        tokio::spawn(async move {
            run_server(node, server_addr, enable_control, cancel_token, tx_ready).await
        });

        rx_ready.await.unwrap();

        guard
    }

    async fn get_available_port() -> u16 {
        for port in 1025..65535 {
            if is_port_available(port).await {
                return port;
            }
        }

        panic!("Could not find an available port");
    }

    async fn is_port_available(port: u16) -> bool {
        match TcpListener::bind(("127.0.0.1", port)).await {
            Ok(_) => true,
            Err(_) => false,
        }
    }
}
