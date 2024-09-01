use rsnano_node::node::Node;
use serde_json::to_string_pretty;
use std::{sync::Arc, time::Instant};
use rsnano_rpc_messages::UptimeDto;

pub async fn uptime(node: Arc<Node>) -> String {
    let seconds = Instant::now() - node.telemetry.startup_time;
    let uptime = UptimeDto::new(seconds.as_secs());
    to_string_pretty(&uptime).unwrap()
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use test_helpers::System;

    #[test]
    fn uptime() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        node.tokio.block_on(async {
            rpc_client
                .uptime()
                .await
                .unwrap()
        });

        server.abort();
    }
}
