use super::{
    wallet::{AccountVecHandle, WalletHandle},
    wallet_action_thread::{
        WalletActionCallback, WalletActionObserverCallback, WalletActionThreadLock,
    },
    wallet_representatives::WalletRepresentativesLock,
};
use crate::{
    block_processing::BlockProcessorHandle,
    core::{BlockDetailsDto, BlockHandle},
    ledger::datastore::{lmdb::LmdbEnvHandle, LedgerHandle, TransactionHandle},
    representatives::OnlineRepsHandle,
    utils::{ContextWrapper, ThreadPoolHandle},
    work::{DistributedWorkFactoryHandle, WorkThresholdsDto},
    NetworkParamsDto, NodeConfigDto, U256ArrayDto, VoidPointerCallback,
};
use rsnano_core::{work::WorkThresholds, Account, Amount, BlockDetails, BlockHash, Root, WalletId};
use rsnano_node::{
    config::NodeConfig,
    wallets::{Wallet, Wallets, WalletsError, WalletsExt},
    NetworkParams,
};
use std::{
    collections::HashMap,
    ffi::{c_char, c_void, CStr},
    ops::Deref,
    sync::{Arc, MutexGuard},
};
use tracing::warn;

pub struct LmdbWalletsHandle(pub Arc<Wallets>);

impl Deref for LmdbWalletsHandle {
    type Target = Arc<Wallets>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_create(
    enable_voting: bool,
    lmdb: &LmdbEnvHandle,
    ledger: &LedgerHandle,
    node_config: &NodeConfigDto,
    kdf_work: u32,
    work_thresholds: &WorkThresholdsDto,
    distributed_work: &DistributedWorkFactoryHandle,
    network_params: &NetworkParamsDto,
    workers: &ThreadPoolHandle,
    block_processor: &BlockProcessorHandle,
    online_reps: &OnlineRepsHandle,
) -> *mut LmdbWalletsHandle {
    let network_params = NetworkParams::try_from(network_params).unwrap();
    let node_config = NodeConfig::try_from(node_config).unwrap();
    let work = WorkThresholds::from(work_thresholds);
    Box::into_raw(Box::new(LmdbWalletsHandle(Arc::new(
        Wallets::new(
            enable_voting,
            Arc::clone(lmdb),
            Arc::clone(&ledger.0),
            &node_config,
            kdf_work,
            work,
            Arc::clone(distributed_work),
            network_params,
            Arc::clone(workers),
            Arc::clone(block_processor),
            Arc::clone(online_reps),
        )
        .expect("could not create wallet"),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_destroy(handle: *mut LmdbWalletsHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_get_wallet_ids(
    handle: *mut LmdbWalletsHandle,
    txn: *mut TransactionHandle,
    result: *mut U256ArrayDto,
) {
    let wallet_ids = (*handle).0.get_wallet_ids((*txn).as_txn());
    let data = wallet_ids.iter().map(|i| *i.as_bytes()).collect();
    (*result).initialize(data)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_get_block_hash(
    handle: *mut LmdbWalletsHandle,
    txn: *mut TransactionHandle,
    id: *const c_char,
    hash: *mut u8,
) -> bool {
    let id = CStr::from_ptr(id).to_str().unwrap();
    match (*handle).0.get_block_hash((*txn).as_txn(), id) {
        Ok(Some(h)) => {
            h.copy_bytes(hash);
            true
        }
        Ok(None) => {
            BlockHash::zero().copy_bytes(hash);
            true
        }
        Err(_) => false,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_set_block_hash(
    handle: *mut LmdbWalletsHandle,
    txn: *mut TransactionHandle,
    id: *const c_char,
    hash: *const u8,
) -> bool {
    let id = CStr::from_ptr(id).to_str().unwrap();
    (*handle)
        .0
        .set_block_hash((*txn).as_write_txn(), id, &BlockHash::from_ptr(hash))
        .is_ok()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_clear_send_ids(
    handle: *mut LmdbWalletsHandle,
    txn: *mut TransactionHandle,
) {
    (*handle).0.clear_send_ids((*txn).as_write_txn())
}

pub struct WalletsMutexLockHandle(MutexGuard<'static, HashMap<WalletId, Arc<Wallet>>>);

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_mutex_lock(
    handle: &LmdbWalletsHandle,
) -> *mut WalletsMutexLockHandle {
    let guard = unsafe {
        let guard = handle.0.mutex.lock().unwrap();
        std::mem::transmute::<
            MutexGuard<'_, HashMap<WalletId, Arc<Wallet>>>,
            MutexGuard<'static, HashMap<WalletId, Arc<Wallet>>>,
        >(guard)
    };
    Box::into_raw(Box::new(WalletsMutexLockHandle(guard)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_mutex_try_lock(
    handle: &LmdbWalletsHandle,
) -> *mut WalletsMutexLockHandle {
    match handle.0.mutex.try_lock() {
        Ok(guard) => {
            let guard = unsafe {
                std::mem::transmute::<
                    MutexGuard<'_, HashMap<WalletId, Arc<Wallet>>>,
                    MutexGuard<'static, HashMap<WalletId, Arc<Wallet>>>,
                >(guard)
            };
            Box::into_raw(Box::new(WalletsMutexLockHandle(guard)))
        }
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_mutex_lock_destroy(handle: *mut WalletsMutexLockHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_mutex_lock_size(
    handle: &WalletsMutexLockHandle,
) -> usize {
    handle.0.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_mutex_lock_find(
    handle: &WalletsMutexLockHandle,
    wallet_id: *const u8,
    wallet: *mut *mut WalletHandle,
) -> bool {
    let wallet_id = WalletId::from_ptr(wallet_id);
    match handle.0.get(&wallet_id) {
        Some(w) => {
            *wallet = Box::into_raw(Box::new(WalletHandle(Arc::clone(w))));
            true
        }
        None => false,
    }
}

#[no_mangle]
pub extern "C" fn rsn_lmdb_wallets_mutex_lock_get_all(
    handle: &WalletsMutexLockHandle,
) -> *mut WalletVecHandle {
    let wallets = handle
        .0
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    Box::into_raw(Box::new(WalletVecHandle(wallets)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_mutex_lock_insert(
    handle: &mut WalletsMutexLockHandle,
    wallet_id: *const u8,
    wallet: &WalletHandle,
) {
    handle
        .0
        .insert(WalletId::from_ptr(wallet_id), Arc::clone(&wallet.0));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_lmdb_wallets_mutex_lock_erase(
    handle: &mut WalletsMutexLockHandle,
    wallet_id: *const u8,
) {
    handle.0.remove(&WalletId::from_ptr(wallet_id));
}

pub struct WalletVecHandle(Vec<(WalletId, Arc<Wallet>)>);

#[no_mangle]
pub extern "C" fn rsn_wallet_vec_len(handle: &WalletVecHandle) -> usize {
    handle.0.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_wallet_vec_get(
    handle: &WalletVecHandle,
    index: usize,
    wallet_id: *mut u8,
) -> *mut WalletHandle {
    let (id, wallet) = handle.0.get(index).unwrap();
    id.copy_bytes(wallet_id);
    Box::into_raw(Box::new(WalletHandle(Arc::clone(wallet))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_wallet_vec_destroy(handle: *mut WalletVecHandle) {
    drop(Box::from_raw(handle));
}

pub type ForeachRepresentativeAction = extern "C" fn(*mut c_void, *const u8, *const u8);

#[no_mangle]
pub extern "C" fn rsn_wallets_foreach_representative(
    handle: &mut LmdbWalletsHandle,
    action: ForeachRepresentativeAction,
    action_context: *mut c_void,
    delete_context: VoidPointerCallback,
) {
    let context_wrapper = ContextWrapper::new(action_context, delete_context);
    handle.0.foreach_representative(move |account, prv_key| {
        let ctx = context_wrapper.get_context();
        action(
            ctx,
            account.as_bytes().as_ptr(),
            prv_key.as_bytes().as_ptr(),
        );
    });
}
#[no_mangle]
pub unsafe extern "C" fn rsn_wallets_work_cache_blocking(
    handle: &mut LmdbWalletsHandle,
    wallet: &mut WalletHandle,
    account: *const u8,
    root: *const u8,
) {
    handle.work_cache_blocking(wallet, &Account::from_ptr(account), &Root::from_ptr(root));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_wallets_insert_watch(
    handle: &mut LmdbWalletsHandle,
    wallet_id: *const u8,
    accounts: &AccountVecHandle,
) -> u8 {
    match handle.insert_watch(&WalletId::from_ptr(wallet_id), &accounts) {
        Ok(()) => WalletsError::None as u8,
        Err(e) => e as u8,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_wallets_attempt_password(
    handle: &mut LmdbWalletsHandle,
    wallet_id: *const u8,
    password: *const c_char,
) -> u8 {
    match handle.attempt_password(
        &WalletId::from_ptr(wallet_id),
        CStr::from_ptr(password).to_string_lossy(),
    ) {
        Ok(()) => WalletsError::None as u8,
        Err(e) => e as u8,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_wallets_lock(
    handle: &mut LmdbWalletsHandle,
    wallet_id: *const u8,
) -> u8 {
    match handle.lock(&WalletId::from_ptr(wallet_id)) {
        Ok(()) => WalletsError::None as u8,
        Err(e) => e as u8,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_wallets_valid_password(
    handle: &mut LmdbWalletsHandle,
    wallet_id: *const u8,
    valid: *mut bool,
) -> u8 {
    match handle.valid_password(&WalletId::from_ptr(wallet_id)) {
        Ok(val) => {
            *valid = val;
            WalletsError::None as u8
        }
        Err(e) => e as u8,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_wallets_rekey(
    handle: &mut LmdbWalletsHandle,
    wallet_id: *const u8,
    password: *const c_char,
) -> u8 {
    let password = CStr::from_ptr(password).to_string_lossy();
    match handle.rekey(&WalletId::from_ptr(wallet_id), password) {
        Ok(()) => WalletsError::None as u8,
        Err(e) => e as u8,
    }
}

#[no_mangle]
pub extern "C" fn rsn_wallets_start(handle: &LmdbWalletsHandle) {
    handle.start();
}

#[no_mangle]
pub extern "C" fn rsn_wallets_stop(handle: &LmdbWalletsHandle) {
    handle.stop();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_wallets_get_delayed_work(
    handle: &LmdbWalletsHandle,
    account: *const u8,
    root: *mut u8,
) {
    handle
        .delayed_work
        .lock()
        .unwrap()
        .get(&Account::from_ptr(account))
        .cloned()
        .unwrap_or_default()
        .copy_bytes(root);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_wallets_erase_delayed_work(
    handle: &LmdbWalletsHandle,
    account: *const u8,
) {
    handle
        .delayed_work
        .lock()
        .unwrap()
        .remove(&Account::from_ptr(account));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_wallets_work_ensure(
    handle: &LmdbWalletsHandle,
    wallet: &WalletHandle,
    account: *const u8,
    root: *const u8,
) {
    handle.work_ensure(
        Arc::clone(wallet),
        Account::from_ptr(account),
        Root::from_ptr(root),
    );
}

#[no_mangle]
pub extern "C" fn rsn_wallets_set_observer(
    handle: &mut LmdbWalletsHandle,
    observer: WalletActionObserverCallback,
    context: *mut c_void,
    delete_context: VoidPointerCallback,
) {
    let context_wrapper = ContextWrapper::new(context, delete_context);
    let wrapped_observer = Box::new(move |active| {
        let ctx = context_wrapper.get_context();
        observer(ctx, active);
    });
    handle.set_observer(wrapped_observer);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_wallets_queue_wallet_action(
    handle: &LmdbWalletsHandle,
    amount: *const u8,
    wallet: &WalletHandle,
    action: WalletActionCallback,
    context: *mut c_void,
    delete_context: VoidPointerCallback,
) {
    let amount = Amount::from_ptr(amount);
    let context_wrapper = ContextWrapper::new(context, delete_context);
    let wrapped_action = Box::new(move |wallet| {
        let ctx = context_wrapper.get_context();
        action(ctx, Box::into_raw(Box::new(WalletHandle(wallet))))
    });

    handle
        .wallet_actions
        .queue_wallet_action(amount, Arc::clone(&wallet.0), wrapped_action)
}

#[no_mangle]
pub extern "C" fn rsn_wallets_actions_size(handle: &LmdbWalletsHandle) -> usize {
    handle.wallet_actions.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_wallets_actions_lock(
    handle: &LmdbWalletsHandle,
) -> *mut WalletActionThreadLock {
    let guard = handle.wallet_actions.lock();
    Box::into_raw(Box::new(WalletActionThreadLock(guard)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_wallets_action_complete(
    handle: &LmdbWalletsHandle,
    wallet: &WalletHandle,
    block: *mut BlockHandle,
    account: *const u8,
    generate_work: bool,
    details: &BlockDetailsDto,
) -> i32 {
    let block = if block.is_null() {
        None
    } else {
        Some(Arc::clone((*block).deref()))
    };
    match handle.action_complete(
        Arc::clone(wallet),
        block,
        Account::from_ptr(account),
        generate_work,
        &BlockDetails::try_from(details).unwrap(),
    ) {
        Ok(_) => 0,
        Err(e) => {
            warn!("action complete failed: {:?}", e);
            -1
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_wallets_representatives_lock(
    handle: &LmdbWalletsHandle,
) -> *mut WalletRepresentativesLock {
    let guard = handle.representatives.lock().unwrap();
    WalletRepresentativesLock::new(guard)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_wallets_compute_reps(handle: &LmdbWalletsHandle) {
    handle.compute_reps();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_wallets_ongoing_compute_reps(handle: &LmdbWalletsHandle) {
    handle.ongoing_compute_reps();
}
