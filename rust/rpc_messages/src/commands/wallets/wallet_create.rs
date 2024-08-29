use crate::RpcCommand;

impl RpcCommand {
    pub fn wallet_create() -> Self {
        Self::WalletCreate
    }
}

#[cfg(test)]
mod tests {
    use crate::RpcCommand;
    use serde_json::to_string_pretty;

    #[test]
    fn serialize_wallet_create_command() {
        assert_eq!(
            to_string_pretty(&RpcCommand::Stop).unwrap(),
            r#"{
  "action": "stop"
}"#
        )
    }

    #[test]
    fn deserialize_wallet_create_command() {
        let cmd = RpcCommand::wallet_create();
        let serialized = serde_json::to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = serde_json::from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }
}
