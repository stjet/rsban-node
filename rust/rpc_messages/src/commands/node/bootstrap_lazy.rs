use crate::RpcCommand;
use rsnano_core::BlockHash;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn bootstrap_lazy(args: BootsrapLazyArgs) -> Self {
        Self::BoostrapLazy(args)
    }
}

impl From<BlockHash> for BootsrapLazyArgs {
    fn from(value: BlockHash) -> Self {
        Self::builder(value).build()
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct BootsrapLazyArgs {
    pub hash: BlockHash,
    pub force: Option<bool>,
    pub id: Option<String>,
}

impl BootsrapLazyArgs {
    pub fn builder(hash: BlockHash) -> BootsrapLazyArgsBuilder {
        BootsrapLazyArgsBuilder {
            args: BootsrapLazyArgs {
                hash,
                force: None,
                id: None,
            },
        }
    }
}

pub struct BootsrapLazyArgsBuilder {
    args: BootsrapLazyArgs,
}

impl BootsrapLazyArgsBuilder {
    pub fn force(mut self) -> Self {
        self.args.force = Some(true);
        self
    }

    pub fn id(mut self, id: String) -> Self {
        self.args.id = Some(id);
        self
    }

    pub fn build(self) -> BootsrapLazyArgs {
        self.args
    }
}
