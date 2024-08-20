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
    only_confirmed: Option<bool>,
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

    let only_confirmed = only_confirmed.unwrap_or(true);

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
    use rsnano_core::DEV_GENESIS_KEY;
    use std::{
        net::{IpAddr, SocketAddr},
        str::FromStr,
        sync::Arc,
        time::Duration,
    };
    use test_helpers::{RpcClient, System};
    use tokio::time::sleep;

    #[tokio::test]
    async fn account_balance_test() -> Result<()> {
        let mut system = System::new();
        let node = system.make_node();

        let rpc_config = RpcConfig::default();

        let ip_addr = IpAddr::from_str(&rpc_config.address)?;
        let socket_addr = SocketAddr::new(ip_addr, rpc_config.port);

        tokio::spawn(run_rpc_server(node.clone(), socket_addr, false));

        sleep(Duration::from_millis(10)).await;

        let node_url = format!("http://[::1]:{}/", rpc_config.port);
        let node_client = Arc::new(RpcClient::new(Url::parse(&node_url)?));

        let result = node_client
            .account_balance(&DEV_GENESIS_KEY.public_key().encode_account())
            .await?;

        assert_eq!(
            result.get("balance").unwrap().as_str().unwrap(),
            String::from("340282366920938463463374607431768211455")
        );

        Ok(())
    }
}
