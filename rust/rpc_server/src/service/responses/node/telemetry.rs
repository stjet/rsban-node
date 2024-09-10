use std::{net::{Ipv6Addr, SocketAddrV6}, sync::Arc};
use rsnano_messages::TelemetryData;
use rsnano_node::node::Node;

pub async fn telemetry(node: Arc<Node>, address: Option<Ipv6Addr>, port: Option<u16>, raw: Option<bool>) -> Result<TelemetryData, anyhow::Error> {
    if address.is_some() || port.is_some() {
        // Handle specific endpoint telemetry
        if let (Some(addr), Some(port)) = (address, port) {
            let endpoint = SocketAddrV6::new(addr, port, 0, 0);

            //let endpoint = Endpoint::new(addr, port);

            // Check if it's a local request
            if addr.is_loopback() && port == node.network.port() {
                return Ok(node.telemetry.local_telemetry())
            } else {
                // Get telemetry for the specific endpoint
                if let Some(telemetry) = node.telemetry.get_telemetry(&endpoint) {
                    Ok(telemetry)
                } else {
                    Err(anyhow::anyhow!("Peer not found"))
                }
            }
        } else {
            return Err(anyhow::anyhow!("Both address and port are required"));
        }
    } else {
        // Handle consolidated or raw telemetry
        let output_raw = raw.unwrap_or(false);

        //if output_raw {
            // Return raw telemetry data
            //let all_telemetries = node.telemetry().get_all_telemetries();
            // Convert all_telemetries to TelemetryDto format
            // This part depends on how you want to structure your TelemetryDto
            // ...
        //} else {
            // Return local telemetry data
            return Ok(node.telemetry.local_telemetry());
        //}
    }
}