use crate::format_error_message;
use rsnano_core::WalletId;
use rsnano_node::{node::Node, wallets::WalletsExt};
use rsnano_rpc_messages::AccountCreateDto;
use serde_json::to_string_pretty;
use std::sync::Arc;

pub async fn account_create(node: Arc<Node>, wallet: WalletId, index: Option<u32>) -> String {
    let result = if let Some(i) = index {
        node.wallets.deterministic_insert_at(&wallet, i, false)
    } else {
        node.wallets.deterministic_insert2(&wallet, false)
    };

    match result {
        Ok(account) => to_string_pretty(&AccountCreateDto::new(account.encode_hex())).unwrap(),
        Err(_) => format_error_message("Wallet error"),
    }
}

#[cfg(test)]
mod tests {
    use rand::{thread_rng, Rng};
    use reqwest::Url;
    use rsnano_core::{PublicKey, WalletId};
    use rsnano_node::wallets::WalletsExt;
    use rsnano_rpc_client::NanoRpcClient;
    use std::{
        net::{IpAddr, SocketAddr},
        str::FromStr,
        sync::Arc,
    };
    use test_helpers::{get_available_port, System};

    use crate::{run_rpc_server, RpcServerConfig};

    #[test]
    fn account_create_index_none() {
        let mut system = System::new();
        let node = system.make_node();

        let port = get_available_port();
        let rpc_server_config = RpcServerConfig::default();
        let ip_addr = IpAddr::from_str(&rpc_server_config.address).unwrap();
        let socket_addr = SocketAddr::new(ip_addr, port);

        let server =
            node.clone()
                .async_rt
                .tokio
                .spawn(run_rpc_server(node.clone(), socket_addr, true));

        let rpc_url = format!("http://[::1]:{}/", port);
        let rpc_client = Arc::new(NanoRpcClient::new(Url::parse(&rpc_url).unwrap()));

        let wallet_id = WalletId::from_bytes(thread_rng().gen());

        node.wallets.create(wallet_id);

        let result = node
            .async_rt
            .tokio
            .block_on(async { rpc_client.account_create(wallet_id, None).await.unwrap() });

        assert!(node
            .wallets
            .exists(&PublicKey::decode_hex(&result.account).unwrap()));

        server.abort();
    }
}
