use crate::RpcCommand;
use rsnano_core::BlockHash;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn block_confirm(hash: BlockHash) -> Self {
        Self::BlockConfirm(BlockConfirmArgs::new(hash))
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct BlockConfirmArgs {
    pub hash: BlockHash
}

impl BlockConfirmArgs {
    pub fn new(hash: BlockHash) -> Self {
        Self { hash }
    }
}

#[cfg(test)]
mod tests {
    use crate::RpcCommand;
    use rsnano_core::BlockHash;
    use serde_json::{from_str, to_string_pretty};

    #[test]
    fn serialize_account_block_count_command() {
        assert_eq!(
            serde_json::to_string_pretty(&RpcCommand::block_confirm(BlockHash::zero())).unwrap(),
            r#"{
  "action": "block_confirm",
  "hash": "0000000000000000000000000000000000000000000000000000000000000000"
}"#
        )
    }

    #[test]
    fn derialize_account_block_count_command() {
        let cmd = RpcCommand::block_confirm(BlockHash::zero());
        let serialized = to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized)
    }
}
