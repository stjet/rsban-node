use crate::RpcCommand;
use rsnano_core::WalletId;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn wallet_ledger(args: WalletLedgerArgs) -> Self {
        Self::WalletLedger(args)
    }
}

impl From<WalletId> for WalletLedgerArgs {
    fn from(value: WalletId) -> Self {
        Self::builder(value).build()
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WalletLedgerArgs {
    pub wallet: WalletId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub representative: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receivable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_since: Option<u64>,
}

impl WalletLedgerArgs {
    pub fn builder(wallet: WalletId) -> WalletLedgerArgsBuilder {
        WalletLedgerArgsBuilder {
            args: WalletLedgerArgs {
                wallet,
                representative: None,
                weight: None,
                receivable: None,
                modified_since: None,
            },
        }
    }
}

pub struct WalletLedgerArgsBuilder {
    args: WalletLedgerArgs,
}

impl WalletLedgerArgsBuilder {
    pub fn representative(mut self) -> Self {
        self.args.representative = Some(true);
        self
    }

    pub fn receivable(mut self) -> Self {
        self.args.receivable = Some(true);
        self
    }

    pub fn weight(mut self) -> Self {
        self.args.weight = Some(true);
        self
    }

    pub fn modified_since(mut self, value: u64) -> Self {
        self.args.modified_since = Some(value);
        self
    }

    pub fn build(self) -> WalletLedgerArgs {
        self.args
    }
}
