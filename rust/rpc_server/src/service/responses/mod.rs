mod ledger;
mod node;
mod utils;
mod wallets;

pub use ledger::*;
pub use node::*;
pub use utils::*;
pub use wallets::*;

#[cfg(test)]
mod test_helpers {
    use crate::run_rpc_server;
    use reqwest::Url;
    use rsnano_node::node::Node;
    use rsnano_rpc_client::NanoRpcClient;
    use std::{
        net::{IpAddr, Ipv6Addr, SocketAddr},
        sync::Arc,
    };
    use test_helpers::get_available_port;

    pub(crate) fn setup_rpc_client_and_server(
        node: Arc<Node>,
        enable_control: bool,
    ) -> (
        Arc<NanoRpcClient>,
        tokio::task::JoinHandle<Result<(), anyhow::Error>>,
    ) {
        let port = get_available_port();
        let socket_addr = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port);

        let server = node
            .tokio
            .spawn(run_rpc_server(node.clone(), socket_addr, enable_control));

        let rpc_url = format!("http://[::1]:{}/", port);
        let rpc_client = Arc::new(NanoRpcClient::new(Url::parse(&rpc_url).unwrap()));

        (rpc_client, server)
    }
}
