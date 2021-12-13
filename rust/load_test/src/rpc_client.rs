use anyhow::bail;
use anyhow::Result;
use reqwest::Url;
use serde_json::json;
use std::time::Duration;

#[derive(Debug)]
pub struct Account {
    pub private_key: String,
    pub public_key: String,
    pub as_string: String,
}

#[derive(PartialEq, Eq, Debug)]
pub struct AccountInfo {
    pub frontier: String,
    pub block_count: String,
    pub balance: String,
}

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
        let result = self.client
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

    pub async fn send_block(&self, wallet: &str, source: &str, destination: &str) -> Result<String> {
        let request = json!({
            "action": "send",
            "wallet": wallet,
            "source": source,
            "destination": destination,
            "amount": "1"
        });
        let json = self.rpc_request(&request).await?;
        let block = json["block"].as_str().unwrap().to_owned();
        Ok(block)
    }

    pub async fn send_receive(&self, wallet: &str, source: &str, destination: &str) -> Result<()> {
        let block = self.send_block(wallet, source, destination).await?;
        self.receive_block(wallet, destination, &block).await
    }

    pub async fn keepalive_rpc(&self, port: u16) -> Result<()> {
        let request = json!({
            "action": "keepalive",
            "address": "::1",
            "port": port
        });
        self.rpc_request(&request).await?;
        Ok(())
    }

    pub async fn key_create_rpc(&self) -> Result<Account> {
        let request = json!({
            "action": "key_create"
        });
        let json = self.rpc_request(&request).await?;

        let account = Account {
            private_key: json["private"].as_str().unwrap().to_owned(),
            public_key: json["public"].as_str().unwrap().to_owned(),
            as_string: json["account"].as_str().unwrap().to_owned(),
        };

        Ok(account)
    }

    pub async fn wallet_create_rpc(&self) -> Result<String> {
        let request = json!({
            "action": "wallet_create"
        });
        let json = self.rpc_request(&request).await?;
        Ok(json["wallet"].as_str().unwrap().to_owned())
    }

    pub async fn wallet_add_rpc(&self, wallet: &str, prv_key: &str) -> Result<()> {
        let request = json!({
            "action": "wallet_add",
            "wallet": wallet,
            "key": prv_key,
        });
        self.rpc_request(&request).await?;
        Ok(())
    }

    pub async fn stop_rpc(&self) -> Result<()> {
        let request = json!({
            "action": "stop"
        });
        self.rpc_request(&request).await?;
        Ok(())
    }

    pub async fn account_info_rpc(&self, account: &str) -> Result<AccountInfo> {
        let request = json!({
            "action": "account_info",
            "account": account
        });

        let json = self.rpc_request(&request).await?;

        Ok(AccountInfo {
            frontier: json["frontier"].as_str().unwrap().to_owned(),
            block_count: json["block_count"].as_str().unwrap().to_owned(),
            balance: json["balance"].as_str().unwrap().to_owned(),
        })
    }
}