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
    use rsnano_core::{Account, Amount, BlockEnum, StateBlock, DEV_GENESIS_KEY};
    use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
    use rsnano_node::node::Node;
    use rsnano_rpc_client::NanoRpcClient;
    use std::{
        net::{IpAddr, Ipv6Addr, SocketAddr},
        sync::Arc, time::Duration,
    };
    use test_helpers::{assert_timely_msg, get_available_port};

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

    pub(crate) fn send_block(node: Arc<Node>, account: Account, amount: Amount) -> BlockEnum {
        let transaction = node.ledger.read_txn();
        let previous = node.ledger.any().account_head(&transaction, &*DEV_GENESIS_ACCOUNT).unwrap_or(*DEV_GENESIS_HASH);
        let balance = node.ledger.any().account_balance(&transaction, &*DEV_GENESIS_ACCOUNT).unwrap_or(Amount::MAX);

        let send = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            previous,
            *DEV_GENESIS_PUB_KEY,
            balance - amount,
            account.into(),
            &DEV_GENESIS_KEY,
            node.work_generate_dev(previous.into()),
        ));

        node.process_active(send.clone());
        assert_timely_msg(
            Duration::from_secs(5),
            || node.active.active(&send),
            "not active on node",
        );

        send
    }
}
