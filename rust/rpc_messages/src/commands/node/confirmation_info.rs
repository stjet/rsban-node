use crate::RpcCommand;
use rsnano_core::QualifiedRoot;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn confirmation_info(args: ConfirmationInfoArgs) -> Self {
        Self::ConfirmationInfo(args)
    }
}

impl From<QualifiedRoot> for ConfirmationInfoArgs {
    fn from(value: QualifiedRoot) -> Self {
        Self::builder(value).build()
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ConfirmationInfoArgs {
    pub root: QualifiedRoot,
    pub contents: Option<bool>,
    pub representatives: Option<bool>,
}

impl ConfirmationInfoArgs {
    pub fn builder(root: QualifiedRoot) -> ConfirmationInfoArgsBuilder {
        ConfirmationInfoArgsBuilder {
            args: ConfirmationInfoArgs {
                root,
                contents: None,
                representatives: None,
            },
        }
    }
}

pub struct ConfirmationInfoArgsBuilder {
    args: ConfirmationInfoArgs,
}

impl ConfirmationInfoArgsBuilder {
    pub fn without_contents(mut self) -> Self {
        self.args.contents = Some(false);
        self
    }

    pub fn include_representatives(mut self) -> Self {
        self.args.representatives = Some(true);
        self
    }

    pub fn build(self) -> ConfirmationInfoArgs {
        self.args
    }
}
