use crate::RpcCommand;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn block_count() -> Self {
        Self::BlockCount
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct BlockCountDto {
    count: u64,
    unchecked: u64,
    cemented: u64,
}

impl BlockCountDto {
    pub fn new(count: u64, unchecked: u64, cemented: u64) -> Self {
        Self {
            count,
            unchecked,
            cemented,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::RpcCommand;
    use serde_json::{from_str, to_string_pretty};

    #[test]
    fn serialize_account_block_count_command() {
        assert_eq!(
            serde_json::to_string_pretty(&RpcCommand::block_count()).unwrap(),
            r#"{
  "action": "block_count"
}"#
        )
    }

    #[test]
    fn derialize_account_block_count_command() {
        let cmd = RpcCommand::block_count();
        let serialized = to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }
}
