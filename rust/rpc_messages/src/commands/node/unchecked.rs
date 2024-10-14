use crate::RpcCommand;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn unchecked(count: u64) -> Self {
        Self::Unchecked(UncheckedArgs::new(count))
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]

pub struct UncheckedArgs {
    pub count: u64,
}

impl UncheckedArgs {
    pub fn new(count: u64) -> Self {
        Self { count }
    }
}
