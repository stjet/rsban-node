use crate::RpcCommand;
use rsnano_core::{BlockHash, HashOrAccount, JsonBlock};
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn unchecked_keys(key: HashOrAccount, count: u64) -> Self {
        Self::UncheckedKeys(UncheckedKeysArgs { key, count })
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]

pub struct UncheckedKeysArgs {
    pub key: HashOrAccount,
    pub count: u64,
}

