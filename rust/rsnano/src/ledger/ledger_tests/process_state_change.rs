use crate::{
    core::{Account, Block, BlockBuilder, BlockDetails, BlockEnum, Epoch, Link},
    ledger::{ledger_tests::AccountBlockFactory, DEV_GENESIS_KEY},
    DEV_CONSTANTS, DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH,
};

use super::LedgerContext;

#[test]
fn save_block() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = AccountBlockFactory::genesis(&ctx.ledger);

    let mut change = genesis.state_change(txn.txn(), Account::from(1)).build();
    ctx.ledger.process(txn.as_mut(), &mut change).unwrap();

    let BlockEnum::State(loaded_block) = ctx.ledger.store.block().get(txn.txn(), &change.hash()).unwrap() else { panic!("not a state block!")};
    assert_eq!(loaded_block, change);
    assert_eq!(loaded_block.sideband().unwrap(), change.sideband().unwrap());
}

#[test]
fn create_sideband() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = AccountBlockFactory::genesis(&ctx.ledger);

    let mut change = genesis.state_change(txn.txn(), Account::from(1)).build();
    ctx.ledger.process(txn.as_mut(), &mut change).unwrap();

    let sideband = change.sideband().unwrap();
    assert_eq!(sideband.height, 2);
    assert_eq!(
        sideband.details,
        BlockDetails::new(Epoch::Epoch0, false, false, false)
    );
}

#[test]
fn update_vote_weight() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = AccountBlockFactory::genesis(&ctx.ledger);
    let rep_account = Account::from(1);
    let mut change = genesis.state_change(txn.txn(), rep_account).build();
    ctx.ledger.process(txn.as_mut(), &mut change).unwrap();

    let weight = ctx.ledger.weight(&rep_account);
    assert_eq!(weight, change.balance());
}

fn change_genesis_representative(rep_account: Account) -> crate::core::StateBlock {
    BlockBuilder::state()
        .account(*DEV_GENESIS_ACCOUNT)
        .previous(*DEV_GENESIS_HASH)
        .representative(rep_account)
        .balance(DEV_CONSTANTS.genesis_amount)
        .link(Link::zero())
        .sign(&DEV_GENESIS_KEY)
        .build()
}
