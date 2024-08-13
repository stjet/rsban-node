use super::helpers::System;
use crate::tests::helpers::assert_timely_eq;
use anyhow::{bail, Result};
use reqwest::Url;
use rsnano_core::{Amount, BlockEnum, StateBlock, DEV_GENESIS_KEY};
use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH};
use rsnano_rpc::{run_rpc_server, NodeRpcRequest, RpcConfig, RpcRequest};
use serde_json::{json, Value};
use std::{
    net::{IpAddr, Ipv6Addr, SocketAddr},
    str::FromStr,
    sync::Arc,
    time::Duration,
};
use tokio::time::sleep;

pub struct RpcClient {
    url: Url,
    client: reqwest::Client,
}

impl RpcClient {
    pub fn new(url: Url) -> Self {
        Self {
            url,
            client: reqwest::ClientBuilder::new()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap(),
        }
    }

    async fn rpc_request(&self, request: &serde_json::Value) -> Result<serde_json::Value> {
        let result = self
            .client
            .post(self.url.clone())
            .json(request)
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;

        if let Some(error) = result.get("error") {
            bail!("node returned error: {}", error);
        }

        Ok(result)
    }

    pub async fn receive_block(&self, wallet: &str, destination: &str, block: &str) -> Result<()> {
        let request = json!({
            "action": "receive",
            "wallet": wallet,
            "account": destination,
            "block": block
        });
        self.rpc_request(&request).await?;
        Ok(())
    }

    pub async fn account_balance(&self, destination: &str) -> Result<Value> {
        let request = json!({
            "action": "account_balance",
            "account": destination,
        });
        Ok(self.rpc_request(&request).await?)
    }
}

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
