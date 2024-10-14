use rsnano_node::Node;
use rsnano_rpc_messages::UptimeDto;
use serde_json::to_string_pretty;
use std::{sync::Arc, time::Instant};

pub async fn uptime(node: Arc<Node>) -> String {
    let seconds = Instant::now() - node.telemetry.startup_time;
    let uptime = UptimeDto::new(seconds.as_secs());
    to_string_pretty(&uptime).unwrap()
}
