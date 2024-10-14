use crate::RpcCommand;
use rsnano_core::{BlockHash, WorkNonce};
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn work_validate(work: WorkNonce, hash: BlockHash) -> Self {
        Self::WorkValidate(WorkValidateArgs::new(work, hash))
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]

pub struct WorkValidateArgs {
    pub work: WorkNonce,
    pub hash: BlockHash,
}

impl WorkValidateArgs {
    pub fn new(work: WorkNonce, hash: BlockHash) -> Self {
        Self { work, hash }
    }
}
