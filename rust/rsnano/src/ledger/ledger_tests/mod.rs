mod ledger_context;
use std::ops::Deref;

pub(crate) use ledger_context::LedgerContext;

mod test_contexts;
pub(crate) use test_contexts::*;

use crate::{
    core::{BlockEnum, BlockHash, QualifiedRoot, Root},
    DEV_GENESIS, DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH,
};

mod empty_ledger;
mod process_change;
mod process_open;
mod process_receive;
mod process_send;
mod rollback_change;
mod rollback_open;
mod rollback_receive;
mod rollback_send;

#[test]
fn ledger_successor() {
    let ctx = LedgerWithSendBlock::new();
    assert_eq!(
        ctx.ledger().successor(
            ctx.txn.txn(),
            &QualifiedRoot::new(Root::zero(), *DEV_GENESIS_HASH)
        ),
        Some(BlockEnum::Send(ctx.send_block))
    );
}

#[test]
fn ledger_successor_genesis() {
    let ctx = LedgerWithSendBlock::new();
    let genesis = DEV_GENESIS.read().unwrap().clone();
    assert_eq!(
        ctx.ledger().successor(
            ctx.txn.txn(),
            &QualifiedRoot::new(DEV_GENESIS_ACCOUNT.deref().into(), BlockHash::zero())
        ),
        Some(genesis)
    );
}
