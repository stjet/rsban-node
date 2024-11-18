use rsnano_core::WalletId;
use rsnano_node::wallets::WalletsExt;
use test_helpers::System;

#[test]
fn open_create() {
    let mut system = System::new();
    let node = system.make_node();
    assert_eq!(node.wallets.mutex.lock().unwrap().len(), 1); // it starts out with a default wallet
    let id = WalletId::random();
    assert_eq!(node.wallets.wallet_exists(&id), false);
    node.wallets.create(id);
    assert_eq!(node.wallets.wallet_exists(&id), true);
}
