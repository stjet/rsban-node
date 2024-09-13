use rsnano_core::{Amount, WalletId};
use serde::{Deserialize, Serialize};
use crate::RpcCommand;

impl RpcCommand {
    pub fn wallet_receivable(
        wallet: WalletId,
        count: u64,
        threshold: Option<Amount>,
        source: Option<bool>,
        min_version: Option<bool>,
        include_only_confirmed: Option<bool>
    ) -> Self {
        Self::WalletReceivable(WalletReceivableArgs {
            wallet,
            count,
            threshold,
            source,
            min_version,
            include_only_confirmed
        })
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WalletReceivableArgs {
    pub wallet: WalletId,
    pub count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<Amount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_version: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_only_confirmed: Option<bool>
}

impl WalletReceivableArgs {
    pub fn new(
        wallet: WalletId,
        count: u64,
        threshold: Option<Amount>,
        source: Option<bool>,
        min_version: Option<bool>,
        include_only_confirmed: Option<bool>
    ) -> Self {
        Self {
            wallet,
            count,
            threshold,
            source,
            min_version,
            include_only_confirmed
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::RpcCommand;
    use rsnano_core::WalletId;
    use serde_json::to_string_pretty;

    #[test]
    fn serialize_wallet_receivable_command() {
        assert_eq!(
            to_string_pretty(&RpcCommand::wallet_receivable(WalletId::zero(), 1, None, None, None, None)).unwrap(),
            r#"{
  "action": "wallet_receivable",
  "wallet": "0000000000000000000000000000000000000000000000000000000000000000",
  "count": 1
}"#
        )
    }

    #[test]
    fn deserialize_wallet_receivable_command() {
        let cmd = RpcCommand::wallet_receivable(WalletId::zero(), 1, None, None, None, None);
        let serialized = serde_json::to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = serde_json::from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }
}