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

    let only_confirmed = match only_confirmed.as_deref() {
        Some("true") => true,
        Some("false") => false,
        _ => true,
    };

    let balance = match if only_confirmed {
        node.ledger.confirmed().account_balance(&tx, &account)
    } else {
        node.ledger.any().account_balance(&tx, &account)
    } {
        Some(balance) => balance,
        None => return format_error_message("Account not found"),
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
    use std::net::{IpAddr, SocketAddr};
    use std::str::FromStr;
    use std::sync::Arc;
    use std::time::Duration;
    use test_helpers::{assert_timely_msg, get_available_port, RpcClient, System};

    fn setup_node_and_rpc_server() -> (
        Arc<Node>,
        Arc<RpcClient>,
        tokio::task::JoinHandle<Result<(), anyhow::Error>>,
    ) {
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
        let port = get_available_port();
        let socket_addr = SocketAddr::new(ip_addr, port);

        let server = node.clone().async_rt.tokio.spawn(run_rpc_server(
            node.clone(),
            socket_addr,
            rpc_config.enable_control,
        ));

        let rpc_url = format!("http://[::1]:{}/", port);
        let rpc_client = Arc::new(RpcClient::new(Url::parse(&rpc_url).unwrap()));

        (node, rpc_client, server)
    }

    #[test]
    fn account_balance_only_confirmed_none() -> Result<()> {
        let (node, rpc_client, server) = setup_node_and_rpc_server();

        let result = node.async_rt.tokio.block_on(async {
            rpc_client
                .account_balance(&DEV_GENESIS_KEY.public_key().encode_account(), None)
                .await
                .unwrap()
        });

        assert_eq!(
            result.get("balance").unwrap().as_str().unwrap(),
            "340282366920938463463374607431768211455"
        );

        assert_eq!(result.get("pending").unwrap().as_str().unwrap(), "0");

        assert_eq!(result.get("receivable").unwrap().as_str().unwrap(), "0");

        server.abort();

        Ok(())
    }

    #[test]
    fn account_balance_only_confirmed_true() -> Result<()> {
        let (node, rpc_client, server) = setup_node_and_rpc_server();

        let result = node.async_rt.tokio.block_on(async {
            rpc_client
                .account_balance(&DEV_GENESIS_KEY.public_key().encode_account(), Some("true"))
                .await
                .unwrap()
        });

        assert_eq!(
            result.get("balance").unwrap().as_str().unwrap(),
            "340282366920938463463374607431768211455"
        );

        assert_eq!(result.get("pending").unwrap().as_str().unwrap(), "0");

        assert_eq!(result.get("receivable").unwrap().as_str().unwrap(), "0");

        server.abort();

        Ok(())
    }

    #[test]
    fn account_balance_only_confirmed_false() -> Result<()> {
        let (node, rpc_client, server) = setup_node_and_rpc_server();

        let result = node.async_rt.tokio.block_on(async {
            rpc_client
                .account_balance(
                    &DEV_GENESIS_KEY.public_key().encode_account(),
                    Some("false"),
                )
                .await
                .unwrap()
        });

        assert_eq!(
            result.get("balance").unwrap().as_str().unwrap(),
            String::from("340282366920938463463374607431768211454")
        );

        assert_eq!(
            result.get("pending").unwrap().as_str().unwrap(),
            String::from("1")
        );

        assert_eq!(
            result.get("receivable").unwrap().as_str().unwrap(),
            String::from("1")
        );

        server.abort();

        Ok(())
    }

    #[test]
    fn account_balance_only_confirmed_invalid() -> Result<()> {
        let (node, rpc_client, server) = setup_node_and_rpc_server();

        let result = node.async_rt.tokio.block_on(async {
            rpc_client
                .account_balance(
                    &DEV_GENESIS_KEY.public_key().encode_account(),
                    Some("invalid"),
                )
                .await
                .unwrap()
        });

        assert_eq!(
            result.get("balance").unwrap().as_str().unwrap(),
            String::from("340282366920938463463374607431768211455")
        );

        assert_eq!(
            result.get("pending").unwrap().as_str().unwrap(),
            String::from("0")
        );

        assert_eq!(
            result.get("receivable").unwrap().as_str().unwrap(),
            String::from("0")
        );

        server.abort();

        Ok(())
    }
}
