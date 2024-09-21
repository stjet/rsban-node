use std::{net::{Ipv6Addr, SocketAddrV6}, sync::Arc};
use rsnano_node::node::Node;
use rsnano_rpc_messages::ErrorDto;
use serde_json::{to_string_pretty, Value, json};

pub async fn telemetry(node: Arc<Node>, address: Option<Ipv6Addr>, port: Option<u16>, raw: Option<bool>) -> String {
    if address.is_some() || port.is_some() {
        let address = address.unwrap();
        let port = port.unwrap();
        let endpoint = SocketAddrV6::new(address, port, 0, 0);

        if address.is_loopback() && port == node.network.port() {
            return to_string_pretty(&node.telemetry.local_telemetry()).unwrap();
        }

        match node.telemetry.get_telemetry(&endpoint.into()) {
            Some(data) => to_string_pretty(&data).unwrap(),
            None => to_string_pretty(&ErrorDto::new("Peer not found".to_string())).unwrap()
        }          
    } else {
        let output_raw = raw.unwrap_or(false);

        if output_raw {
            let all_telemetries = node.telemetry.get_all_telemetries();
            let metrics: Vec<Value> = all_telemetries.iter().map(|(endpoint, telemetry)| {
                let mut telemetry_json = serde_json::to_value(telemetry).unwrap();
                telemetry_json.as_object_mut().unwrap().insert("address".to_string(), json!(endpoint.ip().to_string()));
                telemetry_json.as_object_mut().unwrap().insert("port".to_string(), json!(endpoint.port()));
                telemetry_json
            }).collect();

            to_string_pretty(&json!({ "metrics": metrics })).unwrap()
        } else {
            to_string_pretty(&node.telemetry.local_telemetry()).unwrap()
        }
    }
}

#[cfg(test)]
mod tests {
    use test_helpers::{assert_timely_eq, establish_tcp, System};
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use std::time::Duration;
    use rsnano_messages::TelemetryData;

    #[test]
    fn telemetry_single() {
        let mut system = System::new();
        let node = system.build_node().finish();
        let peer = system.build_node().finish();
        establish_tcp(&node, &peer);
        
        // Wait until peers are stored
        assert_timely_eq(
            Duration::from_secs(10),
            || node.store.peer.count(&node.store.tx_begin_read()),
            1
        );

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        // Test with valid local address
        let response = node.tokio.block_on(async {
            rpc_client.telemetry(Some(*node.tcp_listener.local_address().ip()), Some(node.tcp_listener.local_address().port()), None).await
        });
        assert!(response.is_ok());
        assert!(matches!(response.unwrap().metrics[0], TelemetryData { .. }));

        // Test with invalid address
        let response = node.tokio.block_on(async {
            rpc_client.telemetry(Some("::1".parse().unwrap()), Some(65), None).await
        });
        assert!(response.is_err());
        assert_eq!(response.unwrap_err().to_string(), "node returned error: \"Peer not found\"");

        // Test with missing address (should return local telemetry)
        let response = node.tokio.block_on(async {
            rpc_client.telemetry(None, None, None).await
        });
        assert!(response.is_ok());
        assert!(matches!(response.unwrap().metrics[0], TelemetryData { .. }));
        
        server.abort();
    }

    #[test]
    fn telemetry_all() {
        let mut system = System::new();
        let node = system.build_node().finish();
        let peer = system.build_node().finish();
        establish_tcp(&node, &peer);
        
        // Wait until peers are stored
        assert_timely_eq(
            Duration::from_secs(10),
            || node.store.peer.count(&node.store.tx_begin_read()),
            1
        );

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        // Test without raw flag (should return local telemetry)
        let response = node.tokio.block_on(async {
            rpc_client.telemetry(None, None, None).await
        });
        assert!(response.is_ok());
        let local_telemetry = response.unwrap();
        assert!(matches!(local_telemetry.metrics[0], TelemetryData { .. }));

        // Test with raw flag
        let response = node.tokio.block_on(async {
            rpc_client.telemetry(None, None, Some(true)).await
        });
        assert!(response.is_ok());
        let raw_response = response.unwrap();
        
        assert_eq!(raw_response.metrics.len(), 1);

        let peer_telemetry = &raw_response.metrics[0];
        let local_telemetry = node.telemetry.local_telemetry();
        assert_eq!(peer_telemetry.genesis_block, local_telemetry.genesis_block);

        // TODO: Verify the endpoint matches a known peer
        //let endpoint = format!("[{}]:{}", peer_telemetry.p, peer_telemetry["port"]);
        //assert!(!node.network.info.try_read().unwrap().find_channels_by_peering_addr(&endpoint.parse().unwrap()).is_empty());
        
        server.abort();
    }
}