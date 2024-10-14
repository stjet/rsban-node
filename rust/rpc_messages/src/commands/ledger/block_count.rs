use crate::RpcCommand;

impl RpcCommand {
    pub fn block_count() -> Self {
        Self::BlockCount
    }
}

#[cfg(test)]
mod tests {
    use crate::RpcCommand;
    use serde_json::{from_str, to_string_pretty};

    #[test]
    fn serialize_account_block_count_command() {
        assert_eq!(
            serde_json::to_string_pretty(&RpcCommand::BlockCount).unwrap(),
            r#"{
  "action": "block_count"
}"#
        )
    }

    #[test]
    fn derialize_account_block_count_command() {
        let cmd = RpcCommand::BlockCount;
        let serialized = to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }
}
