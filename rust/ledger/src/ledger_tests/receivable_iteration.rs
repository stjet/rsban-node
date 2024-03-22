use crate::LedgerContext;
use rsnano_core::{Account, BlockHash, PendingInfo, PendingKey};

#[test]
fn empty() {
    let ctx = LedgerContext::empty();
    let txn = ctx.ledger.read_txn();

    let mut iterator = ctx
        .ledger
        .receivable_lower_bound(&txn, Account::zero(), BlockHash::zero());

    assert_eq!(iterator.next(), None);
}

#[test]
fn find_exact() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let account = Account::from(100);
    let hash = BlockHash::from(200);
    let key = PendingKey::new(account, hash);
    let pending = PendingInfo::create_test_instance();
    ctx.ledger
        .store
        .pending
        .put(&mut txn, &PendingKey::new(1.into(), 1.into()), &pending);
    ctx.ledger.store.pending.put(&mut txn, &key, &pending);
    ctx.ledger
        .store
        .pending
        .put(&mut txn, &PendingKey::new(200.into(), 1.into()), &pending);

    // exact match
    let mut iterator = ctx.ledger.receivable_lower_bound(&txn, account, hash);

    assert_eq!(iterator.next(), Some((key.clone(), pending.clone())));
    assert_eq!(iterator.next(), None);

    // find higher
    let mut iterator = ctx
        .ledger
        .receivable_lower_bound(&txn, account, BlockHash::from(0));

    assert_eq!(iterator.next(), Some((key, pending)));
    assert_eq!(iterator.next(), None);
}
