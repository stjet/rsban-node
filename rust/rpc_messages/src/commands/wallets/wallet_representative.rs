use crate::{RpcCommand, WalletRpcMessage};
use rsnano_core::WalletId;

impl RpcCommand {
    pub fn wallet_representative(wallet: WalletId) -> Self {
        Self::WalletRepresentative(WalletRpcMessage::new(wallet))
    }
}

use rsnano_core::Account;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WalletRepresentativeDto {
    pub representative: Account,
}

impl WalletRepresentativeDto {
    pub fn new(representative: Account) -> Self {
        Self { representative }
    }
}


#[cfg(test)]
mod tests {
    use crate::RpcCommand;
    use rsnano_core::WalletId;
    use serde_json::to_string_pretty;

    #[test]
    fn serialize_wallet_representative_command() {
        assert_eq!(
            to_string_pretty(&RpcCommand::wallet_representative(WalletId::zero())).unwrap(),
            r#"{
  "action": "wallet_representative",
  "wallet": "0000000000000000000000000000000000000000000000000000000000000000"
}"#
        )
    }

    #[test]
    fn deserialize_wallet_representative_command() {
        let cmd = RpcCommand::wallet_representative(WalletId::zero());
        let serialized = serde_json::to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = serde_json::from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }
}
