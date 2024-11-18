use rsnano_core::{
    Account, Amount, KeyDerivationFunction, KeyPair, PublicKey, RawKey, DEV_GENESIS_KEY,
};
use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
use rsnano_node::{
    unique_path,
    wallets::{WalletsError, WalletsExt},
    DEV_NETWORK_PARAMS,
};
use rsnano_store_lmdb::{LmdbEnv, LmdbWalletStore};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use test_helpers::{assert_timely, System};

#[test]
fn no_special_keys_accounts() {
    let mut test_file = unique_path().unwrap();
    test_file.push("wallet.ldb");
    let env = LmdbEnv::new(test_file).unwrap();
    let mut tx = env.tx_begin_write();
    let kdf = KeyDerivationFunction::new(DEV_NETWORK_PARAMS.kdf_work);
    let wallet =
        LmdbWalletStore::new(0, kdf, &mut tx, &DEV_GENESIS_PUB_KEY, &PathBuf::from("0")).unwrap();
    let key = KeyPair::from(42);
    assert!(!wallet.exists(&tx, &key.public_key()));
    wallet.insert_adhoc(&mut tx, &key.private_key());
    assert!(wallet.exists(&tx, &key.public_key()));

    for i in 0..LmdbWalletStore::special_count().number().as_u64() {
        assert!(!wallet.exists(&tx, &i.into()))
    }
}

#[test]
fn no_key() {
    let mut test_file = unique_path().unwrap();
    test_file.push("wallet.ldb");
    let env = LmdbEnv::new(test_file).unwrap();
    let mut tx = env.tx_begin_write();
    let kdf = KeyDerivationFunction::new(DEV_NETWORK_PARAMS.kdf_work);
    let wallet =
        LmdbWalletStore::new(0, kdf, &mut tx, &DEV_GENESIS_PUB_KEY, &PathBuf::from("0")).unwrap();
    assert!(wallet.fetch(&tx, &PublicKey::from(42)).is_err());
    assert!(wallet.valid_password(&tx));
}

#[test]
fn fetch_locked() {
    let mut test_file = unique_path().unwrap();
    test_file.push("wallet.ldb");
    let env = LmdbEnv::new(test_file).unwrap();
    let mut tx = env.tx_begin_write();
    let kdf = KeyDerivationFunction::new(DEV_NETWORK_PARAMS.kdf_work);
    let wallet =
        LmdbWalletStore::new(0, kdf, &mut tx, &DEV_GENESIS_PUB_KEY, &PathBuf::from("0")).unwrap();
    assert!(wallet.valid_password(&tx));
    let key1 = KeyPair::from(42);
    assert_eq!(
        wallet.insert_adhoc(&mut tx, &key1.private_key()),
        key1.public_key()
    );
    let key2 = wallet.deterministic_insert(&mut tx);
    assert!(!key2.is_zero());
    wallet.set_password(&mut tx, RawKey::from(1));
    assert!(wallet.fetch(&tx, &key1.public_key()).is_err());
    assert!(wallet.fetch(&tx, &key2).is_err());
}

#[test]
fn retrieval() {
    let mut test_file = unique_path().unwrap();
    test_file.push("wallet.ldb");
    let env = LmdbEnv::new(test_file).unwrap();
    let mut tx = env.tx_begin_write();
    let kdf = KeyDerivationFunction::new(DEV_NETWORK_PARAMS.kdf_work);
    let wallet =
        LmdbWalletStore::new(0, kdf, &mut tx, &DEV_GENESIS_PUB_KEY, &PathBuf::from("0")).unwrap();
    let key1 = KeyPair::from(42);
    wallet.insert_adhoc(&mut tx, &key1.private_key());
    let prv1 = wallet.fetch(&tx, &key1.public_key()).unwrap();
    assert_eq!(prv1, key1.private_key());
    wallet.set_password(&mut tx, RawKey::from(123));
    assert!(wallet.fetch(&tx, &key1.public_key()).is_err());
    assert!(!wallet.valid_password(&tx));
}

#[test]
fn empty_iteration() {
    let mut test_file = unique_path().unwrap();
    test_file.push("wallet.ldb");
    let env = LmdbEnv::new(test_file).unwrap();
    let mut tx = env.tx_begin_write();
    let kdf = KeyDerivationFunction::new(DEV_NETWORK_PARAMS.kdf_work);
    let wallet =
        LmdbWalletStore::new(0, kdf, &mut tx, &DEV_GENESIS_PUB_KEY, &PathBuf::from("0")).unwrap();
    assert!(wallet.begin(&tx).is_end());
}

#[test]
fn one_item_iteration() {
    let mut test_file = unique_path().unwrap();
    test_file.push("wallet.ldb");
    let env = LmdbEnv::new(test_file).unwrap();
    let mut tx = env.tx_begin_write();
    let kdf = KeyDerivationFunction::new(DEV_NETWORK_PARAMS.kdf_work);
    let wallet =
        LmdbWalletStore::new(0, kdf, &mut tx, &DEV_GENESIS_PUB_KEY, &PathBuf::from("0")).unwrap();
    let key1 = KeyPair::from(42);
    wallet.insert_adhoc(&mut tx, &key1.private_key());
    let mut it = wallet.begin(&tx);
    while !it.is_end() {
        let (k, v) = it.current().unwrap();
        assert_eq!(*k, key1.public_key());
        let password = wallet.wallet_key(&tx);
        let key = v.key.decrypt(&password, &k.initialization_vector());
        assert_eq!(key, key1.private_key());
        it.next();
    }
}

#[test]
fn two_item_iteration() {
    let mut test_file = unique_path().unwrap();
    test_file.push("wallet.ldb");
    let env = LmdbEnv::new(test_file).unwrap();

    let key1 = KeyPair::new();
    let key2 = KeyPair::new();
    let mut pubs = HashSet::new();
    let mut prvs = HashSet::new();
    let kdf = KeyDerivationFunction::new(DEV_NETWORK_PARAMS.kdf_work);
    {
        let mut tx = env.tx_begin_write();
        let wallet =
            LmdbWalletStore::new(0, kdf, &mut tx, &DEV_GENESIS_PUB_KEY, &PathBuf::from("0"))
                .unwrap();
        wallet.insert_adhoc(&mut tx, &key1.private_key());
        wallet.insert_adhoc(&mut tx, &key2.private_key());
        let mut it = wallet.begin(&tx);
        while let Some((k, v)) = it.current() {
            pubs.insert(*k);
            let password = wallet.wallet_key(&tx);
            let key = v.key.decrypt(&password, &k.initialization_vector());
            prvs.insert(key);
            it.next();
        }
    }
    assert_eq!(pubs.len(), 2);
    assert_eq!(prvs.len(), 2);
    assert!(pubs.contains(&key1.public_key()));
    assert!(prvs.contains(&key1.private_key()));
    assert!(pubs.contains(&key2.public_key()));
    assert!(prvs.contains(&key2.private_key()));
}

#[test]
fn insufficient_spend_one() {
    let mut system = System::new();
    let node = system.make_node();
    let key1 = KeyPair::new();
    node.insert_into_wallet(&DEV_GENESIS_KEY);
    let wallet_id = node.wallets.wallet_ids()[0];
    let _block = node
        .wallets
        .send_action2(
            &wallet_id,
            *DEV_GENESIS_ACCOUNT,
            key1.account(),
            Amount::raw(500),
            0,
            true,
            None,
        )
        .unwrap();

    let error = node
        .wallets
        .send_action2(
            &wallet_id,
            *DEV_GENESIS_ACCOUNT,
            key1.account(),
            Amount::MAX,
            0,
            true,
            None,
        )
        .unwrap_err();
    assert_eq!(error, WalletsError::Generic);
}

#[test]
fn spend_all_one() {
    let mut system = System::new();
    let node = system.make_node();
    node.insert_into_wallet(&DEV_GENESIS_KEY);
    let wallet_id = node.wallets.wallet_ids()[0];
    let key2 = KeyPair::new();
    node.wallets
        .send_action2(
            &wallet_id,
            *DEV_GENESIS_ACCOUNT,
            key2.account(),
            Amount::MAX,
            0,
            true,
            None,
        )
        .unwrap();

    let tx = node.ledger.read_txn();
    let info2 = node
        .ledger
        .any()
        .get_account(&tx, &DEV_GENESIS_ACCOUNT)
        .unwrap();
    assert_ne!(info2.head, *DEV_GENESIS_HASH);
    let block = node.ledger.any().get_block(&tx, &info2.head).unwrap();
    assert_eq!(block.previous(), *DEV_GENESIS_HASH);
    assert_eq!(block.balance(), Amount::zero());
}

#[test]
fn send_async() {
    let mut system = System::new();
    let node = system.make_node();
    node.insert_into_wallet(&DEV_GENESIS_KEY);
    let wallet_id = node.wallets.wallet_ids()[0];
    let key2 = KeyPair::new();
    let block = Arc::new(Mutex::new(None));
    let block2 = block.clone();
    node.wallets
        .send_async(
            wallet_id,
            *DEV_GENESIS_ACCOUNT,
            key2.account(),
            Amount::MAX,
            Box::new(move |b| {
                *block2.lock().unwrap() = Some(b);
            }),
            0,
            true,
            None,
        )
        .unwrap();

    assert_timely(Duration::from_secs(10), || {
        node.balance(&DEV_GENESIS_ACCOUNT).is_zero()
    });
    assert!(block.lock().unwrap().is_some());
}

#[test]
fn spend() {
    let mut system = System::new();
    let node = system.make_node();
    node.insert_into_wallet(&DEV_GENESIS_KEY);
    let wallet_id = node.wallets.wallet_ids()[0];
    let key2 = KeyPair::new();
    // Sending from empty accounts should always be an error.
    // Accounts need to be opened with an open block, not a send block.
    assert!(node
        .wallets
        .send_action2(
            &wallet_id,
            Account::zero(),
            key2.account(),
            Amount::zero(),
            0,
            true,
            None
        )
        .is_err());
    node.wallets
        .send_action2(
            &wallet_id,
            *DEV_GENESIS_ACCOUNT,
            key2.account(),
            Amount::MAX,
            0,
            true,
            None,
        )
        .unwrap();
    assert_eq!(node.balance(&DEV_GENESIS_ACCOUNT), Amount::zero());
}

#[test]
fn partial_spend() {
    let mut system = System::new();
    let node = system.make_node();
    node.insert_into_wallet(&DEV_GENESIS_KEY);
    let wallet_id = node.wallets.wallet_ids()[0];
    let key2 = KeyPair::new();
    node.wallets
        .send_action2(
            &wallet_id,
            *DEV_GENESIS_ACCOUNT,
            key2.account(),
            Amount::raw(500),
            0,
            true,
            None,
        )
        .unwrap();
    assert_eq!(
        node.balance(&DEV_GENESIS_ACCOUNT),
        Amount::MAX - Amount::raw(500)
    );
}

#[test]
fn spend_no_previous() {
    let mut system = System::new();
    let node = system.make_node();
    let wallet_id = node.wallets.wallet_ids()[0];
    {
        node.insert_into_wallet(&DEV_GENESIS_KEY);
        for _ in 0..50 {
            let key = KeyPair::new();
            node.wallets
                .insert_adhoc2(&wallet_id, &key.private_key(), false)
                .unwrap();
        }
    }
    let key2 = KeyPair::new();
    node.wallets
        .send_action2(
            &wallet_id,
            *DEV_GENESIS_ACCOUNT,
            key2.account(),
            Amount::raw(500),
            0,
            true,
            None,
        )
        .unwrap();
    assert_eq!(
        node.balance(&DEV_GENESIS_ACCOUNT),
        Amount::MAX - Amount::raw(500)
    );
}

#[test]
fn find_none() {
    let mut test_file = unique_path().unwrap();
    test_file.push("wallet.ldb");
    let env = LmdbEnv::new(test_file).unwrap();
    let mut tx = env.tx_begin_write();
    let kdf = KeyDerivationFunction::new(DEV_NETWORK_PARAMS.kdf_work);
    let wallet =
        LmdbWalletStore::new(0, kdf, &mut tx, &DEV_GENESIS_PUB_KEY, &PathBuf::from("0")).unwrap();
    assert!(wallet.find(&tx, &PublicKey::from(1000)).is_end());
}
