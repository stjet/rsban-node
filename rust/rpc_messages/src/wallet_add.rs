use rsnano_core::{RawKey, WalletId};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WalletAddRequest {
    pub wallet: WalletId,
    pub key: RawKey,
}

#[cfg(test)]
mod tests {
    use crate::RpcCommand;

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
}
