use primitive_types::U256;
use rsnano_core::{KeyDerivationFunction, KeyPair, PublicKey, RawKey};
use rsnano_ledger::DEV_GENESIS_PUB_KEY;
use rsnano_node::{unique_path, DEV_NETWORK_PARAMS};
use rsnano_store_lmdb::{LmdbEnv, LmdbWalletStore};
use std::path::{Path, PathBuf};

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
