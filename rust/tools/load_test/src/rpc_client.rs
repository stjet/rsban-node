use anyhow::{bail, Result};
use reqwest::Url;
use rsnano_core::{Account, Amount, BlockHash, PublicKey, RawKey, WalletId};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyPairDto {
    pub private_key: RawKey,
    pub public_key: PublicKey,
    pub as_string: Account,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountInfo {
    pub frontier: BlockHash,
    pub block_count: u64,
    pub balance: Amount,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum RpcCommand {
    AccountInfo(AccountInfoCmd),
    WalletAdd(WalletAddCmd),
    Receive(ReceiveCmd),
    Stop,
}

impl RpcCommand {
    pub fn account_info(account: Account) -> Self {
        Self::AccountInfo(AccountInfoCmd { account })
    }

    pub fn wallet_add(wallet_id: WalletId, key: RawKey) -> Self {
        Self::WalletAdd(WalletAddCmd {
            wallet: wallet_id,
            key,
        })
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AccountInfoCmd {
    pub account: Account,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WalletAddCmd {
    pub wallet: WalletId,
    pub key: RawKey,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ReceiveCmd {
    pub wallet: WalletId,
    pub account: Account,
    pub block: String, //todo
}

pub struct NanoRpcClient {
    url: Url,
    client: reqwest::Client,
}

impl NanoRpcClient {
    pub fn new(url: Url) -> Self {
        Self {
            url,
            client: reqwest::ClientBuilder::new()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap(),
        }
    }

    pub async fn account_info(&self, account: Account) -> Result<AccountInfo> {
        let cmd = RpcCommand::account_info(account);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn receive_block(
        &self,
        wallet: WalletId,
        destination: Account,
        block: &str,
    ) -> Result<()> {
        let request = json!({
            "action": "receive",
            "wallet": wallet,
            "account": destination,
            "block": block
        });
        self.rpc_request(&request).await?;
        Ok(())
    }

    pub async fn send_block(
        &self,
        wallet: WalletId,
        source: &str,
        destination: Account,
    ) -> Result<String> {
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

    pub async fn send_receive(
        &self,
        wallet: WalletId,
        source: &str,
        destination: Account,
    ) -> Result<()> {
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

    pub async fn key_create_rpc(&self) -> Result<KeyPairDto> {
        let request = json!({
            "action": "key_create"
        });
        let json = self.rpc_request(&request).await?;
        Ok(serde_json::from_value(json)?)
    }

    pub async fn wallet_create_rpc(&self) -> Result<WalletId> {
        let request = json!({
            "action": "wallet_create"
        });
        let json = self.rpc_request(&request).await?;
        WalletId::decode_hex(json["wallet"].as_str().unwrap())
    }

    pub async fn wallet_add(&self, wallet: WalletId, prv_key: RawKey) -> Result<()> {
        let cmd = RpcCommand::wallet_add(wallet, prv_key);
        self.rpc_request(&cmd).await?;
        Ok(())
    }

    pub async fn stop_rpc(&self) -> Result<()> {
        self.rpc_request(&RpcCommand::Stop).await?;
        Ok(())
    }

    async fn rpc_request<T>(&self, request: &T) -> Result<serde_json::Value>
    where
        T: Serialize,
    {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_account_info_command() {
        assert_eq!(
            serde_json::to_string_pretty(&RpcCommand::account_info(Account::from(123))).unwrap(),
            r#"{
  "action": "account_info",
  "account": "nano_111111111111111111111111111111111111111111111111115uwdgas549"
}"#
        )
    }

    #[test]
    fn serialize_stop_command() {
        assert_eq!(
            serde_json::to_string_pretty(&RpcCommand::Stop).unwrap(),
            r#"{
  "action": "stop"
}"#
        )
    }

    #[test]
    fn serialize_wallet_add_command() {
        assert_eq!(
            serde_json::to_string_pretty(&RpcCommand::wallet_add(1.into(), 2.into())).unwrap(),
            r#"{
  "action": "wallet_add",
  "wallet": "0000000000000000000000000000000000000000000000000000000000000001",
  "key": "0000000000000000000000000000000000000000000000000000000000000002"
}"#
        )
    }

    #[test]
    fn deserialize_account_info_command() {
        let account = Account::from(123);
        let cmd = RpcCommand::account_info(account);
        let serialized = serde_json::to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = serde_json::from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }
}
