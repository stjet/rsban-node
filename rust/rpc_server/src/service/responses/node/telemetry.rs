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

        let response = node
            .tokio
            .block_on(async { rpc_client.telemetry(Some(*node.tcp_listener.local_address().ip()), Some(node.tcp_listener.local_address().port()), None).await.unwrap() 
        });

        /*let response = telemetry(Arc::new(node), Some("not_a_valid_address".parse().unwrap()), None, None).await;
        assert!(response.contains("requires_port_and_address"));

        let response = node
            .tokio
            .block_on(async { rpc_client.telemetry(Some(node.network.endpoint().ip()), Some(node.network.endpoint().port()), None).await.unwrap() 
        });

        // Missing address
        let response = telemetry(Arc::new(node), None, Some(65), None).await;
        assert!(response.contains("requires_port_and_address"));

        // Invalid address
        let response = telemetry(Arc::new(node), Some("not_a_valid_address".parse().unwrap()), Some(65), None).await;
        assert!(response.contains("invalid_ip_address"));

        // Invalid port
        let response = telemetry(Arc::new(node), Some(node.network.endpoint().ip()), Some(0), None).await;
        assert!(response.contains("invalid_port"));

        // Correct address and port
        let response = telemetry(Arc::new(node), Some(node.network.endpoint().ip()), Some(node.network.endpoint().port()), None).await;
        let telemetry_data: Value = serde_json::from_str(&response).unwrap();*/
        
        // Add assertions to compare telemetry_data with node's actual telemetry
        // This part would depend on the specific structure of your telemetry data
        //assert!(telemetry_data.get("version").is_some());
        //assert!(telemetry_data.get("protocol_version").is_some());
        // ... add more assertions as needed
        
        server.abort();
    }
}