use rsnano_core::{RawKey, WalletId};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WalletAddArgs {
    pub wallet: WalletId,
    pub key: RawKey,
    pub work: Option<bool>,
}

impl WalletAddArgs {
    pub fn new(wallet: WalletId, key: RawKey, work: Option<bool>) -> Self {
        Self { wallet, key, work }
    }
}

#[cfg(test)]
mod tests {
    use crate::RpcCommand;
    use serde_json::to_string_pretty;

    #[test]
    fn serialize_wallet_add_command() {
        assert_eq!(
            to_string_pretty(&RpcCommand::wallet_add(1.into(), 2.into(), None)).unwrap(),
            r#"{
  "action": "wallet_add",
  "wallet": "0000000000000000000000000000000000000000000000000000000000000001",
  "key": "0000000000000000000000000000000000000000000000000000000000000002"
}"#
        )
    }
}
