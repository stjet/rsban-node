use crate::server::service::format_error_message;
use rsnano_core::{Account, WalletId};
use rsnano_node::{node::Node, wallets::WalletsExt};
use serde::Serialize;
use serde_json::to_string_pretty;
use std::sync::Arc;

#[derive(Serialize)]
struct AccountCreate {
    account: String,
}

impl AccountCreate {
    fn new(account: String) -> Self {
        Self { account }
    }
}

pub(crate) async fn account_create(
    node: Arc<Node>,
    wallet: String,
    index_str: Option<String>,
) -> String {
    match WalletId::decode_hex(&wallet) {
        Ok(wallet) => {
            let result = if let Some(i) = index_str {
                let index = match i.parse::<u32>() {
                    Ok(idx) => idx,
                    Err(_) => return format_error_message("Invalid index format"),
                };
                node.wallets.deterministic_insert_at(&wallet, index, false)
            } else {
                node.wallets.deterministic_insert2(&wallet, false)
            };

            match result {
                Ok(public_key) => {
                    let account = Account::encode_account(&public_key);
                    to_string_pretty(&AccountCreate::new(account)).unwrap()
                }
                Err(_) => format_error_message("Failed to create account"),
            }
        }
        Err(_) => format_error_message("Bad wallet"),
    }
}

#[cfg(test)]
mod tests {
    use crate::{run_rpc_server, RpcConfig};
    use anyhow::Result;
    use rand::{thread_rng, Rng};
    use reqwest::Url;
    use rsnano_core::{PublicKey, WalletId};
    use rsnano_node::wallets::WalletsExt;
    use std::{
        net::{IpAddr, SocketAddr},
        str::FromStr,
        sync::Arc,
    };
    use test_helpers::{get_available_port, RpcClient, System};

    #[test]
    fn account_create_with_no_index() -> Result<()> {
        let mut system = System::new();
        let node = system.make_node();
        let node_clone = node.clone();

        let rpc_config = RpcConfig::default();
        let ip_addr = IpAddr::from_str(&rpc_config.address).unwrap();
        let port = get_available_port();
        let socket_addr = SocketAddr::new(ip_addr, port);

        let server = node
            .async_rt
            .tokio
            .spawn(run_rpc_server(node_clone, socket_addr, true));

        let node_url = format!("http://[::1]:{}/", port);
        let node_client = Arc::new(RpcClient::new(Url::parse(&node_url).unwrap()));

        let wallet_id = WalletId::from_bytes(thread_rng().gen());
        node.wallets.create(wallet_id.clone());

        let result = node.async_rt.tokio.block_on(async {
            node_client
                .account_create(&wallet_id.encode_hex(), None)
                .await
                .unwrap()
        });

        assert!(node.wallets.exists(
            &PublicKey::decode_account(result.get("account").unwrap().as_str().unwrap()).unwrap()
        ));

        server.abort();

        Ok(())
    }
}
