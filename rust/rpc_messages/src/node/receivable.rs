use rsnano_core::{Account, Amount, BlockHash};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::RpcCommand;

impl RpcCommand {
    pub fn receivable(
        account: Account,
        count: u64,
        threshold: Option<Amount>,
        source: Option<bool>,
        include_active: Option<bool>,
        min_version: Option<bool>,
        sorting: Option<bool>,
        include_only_confirmed: Option<bool>
    ) -> Self {
        Self::Receivable(ReceivableArgs {
            account,
            count,
            threshold,
            source,
            include_active,
            min_version,
            sorting,
            include_only_confirmed
        })
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ReceivableArgs {
    pub account: Account,
    pub count: u64,
    pub threshold: Option<Amount>,
    pub source: Option<bool>,
    pub include_active: Option<bool>,
    pub min_version: Option<bool>,
    pub sorting: Option<bool>,
    pub include_only_confirmed: Option<bool>
}

impl ReceivableArgs {
    pub fn new(
        account: Account,
        count: u64,
        threshold: Option<Amount>,
        source: Option<bool>,
        include_active: Option<bool>,
        min_version: Option<bool>,
        sorting: Option<bool>,
        include_only_confirmed: Option<bool>
    ) -> Self {
        Self {
            account,
            count,
            threshold,
            source,
            include_active,
            min_version,
            sorting,
            include_only_confirmed
        }
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ReceivableDto {
    pub blocks: HashMap<BlockHash, BlockInfo>,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]

pub struct BlockInfo {
    amount: Amount,
    source: Account,
}

impl ReceivableDto {
    pub fn new() -> Self {
        Self {
            blocks: HashMap::new(),
        }
    }

    pub fn add_block(&mut self, hash: BlockHash, amount: Amount, source: Account) {
        self.blocks.insert(hash, BlockInfo { amount, source });
    }
}
