use crate::{
    core::{Account, Amount, Block, BlockDetails, BlockEnum, Epoch, PendingInfo, PendingKey},
    ledger::ledger_tests::{AccountBlockFactory, LedgerContext},
    DEV_CONSTANTS, DEV_GENESIS_ACCOUNT,
};

#[test]
fn save_block() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = AccountBlockFactory::genesis(&ctx.ledger);

    let amount_sent = Amount::new(1);
    let mut block = genesis.send(txn.txn()).amount(amount_sent).build();
    ctx.ledger.process(txn.as_mut(), &mut block).unwrap();

    let BlockEnum::State(loaded_block) = ctx.ledger.store.block().get(txn.txn(), &block.hash()).unwrap() else {panic!("not a state block")};
    assert_eq!(loaded_block.sideband().unwrap(), block.sideband().unwrap());
    assert_eq!(loaded_block, block);
    assert_eq!(
        ctx.ledger.amount(txn.txn(), &block.hash()),
        Some(amount_sent)
    );
}

#[test]
fn update_pending_store() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = AccountBlockFactory::genesis(&ctx.ledger);

    let receiver_account = Account::from(1);
    let amount_sent = Amount::new(1);
    let mut block = genesis
        .send(txn.txn())
        .link(receiver_account)
        .amount(amount_sent)
        .build();
    ctx.ledger.process(txn.as_mut(), &mut block).unwrap();

    let pending_info = ctx
        .ledger
        .store
        .pending()
        .get(txn.txn(), &PendingKey::new(receiver_account, block.hash()))
        .unwrap();

    assert_eq!(
        pending_info,
        PendingInfo {
            source: block.account(),
            amount: amount_sent,
            epoch: Epoch::Epoch0
        }
    );
}

#[test]
fn create_sideband() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = AccountBlockFactory::genesis(&ctx.ledger);

    let mut block = genesis.send(txn.txn()).build();
    ctx.ledger.process(txn.as_mut(), &mut block).unwrap();

    let sideband = block.sideband().unwrap();
    assert_eq!(sideband.height, 2);
    assert_eq!(sideband.account, *DEV_GENESIS_ACCOUNT);
    assert_eq!(
        sideband.details,
        BlockDetails::new(Epoch::Epoch0, true, false, false)
    );
}

#[test]
fn send_and_change_representative() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = AccountBlockFactory::genesis(&ctx.ledger);
    let representative = Account::from(1);
    let amount_sent = DEV_CONSTANTS.genesis_amount - Amount::new(1);
    let mut send = genesis
        .send(txn.txn())
        .amount(amount_sent)
        .representative(representative)
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    assert_eq!(
        ctx.ledger.amount(txn.txn(), &send.hash()).unwrap(),
        amount_sent,
    );
    assert_eq!(ctx.ledger.weight(&DEV_GENESIS_ACCOUNT), Amount::zero());
    assert_eq!(ctx.ledger.weight(&representative), Amount::new(1));
    assert_eq!(
        send.sideband().unwrap().details,
        BlockDetails::new(Epoch::Epoch0, true, false, false)
    );
}
