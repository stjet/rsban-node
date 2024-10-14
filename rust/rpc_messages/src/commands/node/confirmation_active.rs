use crate::RpcCommand;
use rsnano_core::QualifiedRoot;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn confirmation_active(announcements: Option<u64>) -> Self {
        Self::ConfirmationActive(ConfirmationActiveArgs::new(announcements))
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ConfirmationActiveArgs {
    pub announcements: Option<u64>,
}

impl ConfirmationActiveArgs {
    pub fn new(announcements: Option<u64>) -> Self {
        Self { announcements }
    }
}

