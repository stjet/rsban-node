use crate::{common::CountArgs, RpcCommand};
use rsnano_core::{BlockHash, JsonBlock};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

impl RpcCommand {
    pub fn unchecked(count: Option<u64>) -> Self {
        Self::Unchecked(CountArgs { count })
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct UncheckedResponse {
    pub blocks: HashMap<BlockHash, JsonBlock>,
}

impl UncheckedResponse {
    pub fn new(blocks: HashMap<BlockHash, JsonBlock>) -> Self {
        Self { blocks }
    }
}
