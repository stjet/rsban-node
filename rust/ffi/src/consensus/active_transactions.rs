use super::{
    election::{ElectionHandle, ElectionLockHandle},
    election_status::ElectionStatusHandle,
    recently_confirmed_cache::RecentlyConfirmedCacheHandle,
    LocalVoteHistoryHandle,
};
use crate::{
    cementation::ConfirmingSetHandle,
    core::BlockHandle,
    ledger::datastore::LedgerHandle,
    representatives::OnlineRepsHandle,
    utils::{InstantHandle, ThreadPoolHandle},
    wallets::LmdbWalletsHandle,
    NetworkParamsDto, NodeConfigDto,
};
use num_traits::FromPrimitive;
use rsnano_core::{Amount, BlockEnum, BlockHash, QualifiedRoot, Root};
use rsnano_node::{
    config::NodeConfig,
    consensus::{
        ActiveTransactions, ActiveTransactionsData, ActiveTransactionsExt, Election, TallyKey,
    },
};
use std::{
    collections::{BTreeMap, HashMap},
    ops::Deref,
    sync::{Arc, MutexGuard},
};

pub struct ActiveTransactionsHandle(Arc<ActiveTransactions>);

impl Deref for ActiveTransactionsHandle {
    type Target = Arc<ActiveTransactions>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_active_transactions_create(
    network: &NetworkParamsDto,
    online_reps: &OnlineRepsHandle,
    wallets: &LmdbWalletsHandle,
    config: &NodeConfigDto,
    ledger: &LedgerHandle,
    confirming_set: &ConfirmingSetHandle,
    workers: &ThreadPoolHandle,
    history: &LocalVoteHistoryHandle,
) -> *mut ActiveTransactionsHandle {
    Box::into_raw(Box::new(ActiveTransactionsHandle(Arc::new(
        ActiveTransactions::new(
            network.try_into().unwrap(),
            Arc::clone(online_reps),
            Arc::clone(wallets),
            NodeConfig::try_from(config).unwrap(),
            Arc::clone(ledger),
            Arc::clone(confirming_set),
            Arc::clone(workers),
            Arc::clone(history),
        ),
    ))))
}

#[no_mangle]
pub extern "C" fn rsn_active_transactions_request_loop(
    handle: &ActiveTransactionsHandle,
    lock_handle: &mut ActiveTransactionsLockHandle,
    stamp: &InstantHandle,
) {
    let guard = lock_handle.0.take().unwrap();
    let guard = handle.0.request_loop(stamp.0, guard);
    lock_handle.0 = Some(guard);
}

#[no_mangle]
pub extern "C" fn rsn_active_transactions_recently_confirmed(
    handle: &ActiveTransactionsHandle,
) -> *mut RecentlyConfirmedCacheHandle {
    RecentlyConfirmedCacheHandle::new(Arc::clone(&handle.recently_confirmed))
}

#[no_mangle]
pub extern "C" fn rsn_active_transactions_notify_all(handle: &ActiveTransactionsHandle) {
    handle.condition.notify_all();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_active_transactions_destroy(handle: *mut ActiveTransactionsHandle) {
    drop(Box::from_raw(handle))
}

pub struct ActiveTransactionsLockHandle(Option<MutexGuard<'static, ActiveTransactionsData>>);

#[no_mangle]
pub extern "C" fn rsn_active_transactions_lock(
    handle: &ActiveTransactionsHandle,
) -> *mut ActiveTransactionsLockHandle {
    let guard = handle.0.mutex.lock().unwrap();
    let guard = unsafe {
        std::mem::transmute::<
            MutexGuard<ActiveTransactionsData>,
            MutexGuard<'static, ActiveTransactionsData>,
        >(guard)
    };
    Box::into_raw(Box::new(ActiveTransactionsLockHandle(Some(guard))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_active_transactions_lock_destroy(
    handle: *mut ActiveTransactionsLockHandle,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_active_transactions_lock_lock(
    handle: &mut ActiveTransactionsLockHandle,
    active_transactions: &ActiveTransactionsHandle,
) {
    let guard = active_transactions.0.mutex.lock().unwrap();
    let guard = unsafe {
        std::mem::transmute::<
            MutexGuard<ActiveTransactionsData>,
            MutexGuard<'static, ActiveTransactionsData>,
        >(guard)
    };
    handle.0 = Some(guard)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_active_transactions_lock_unlock(
    handle: &mut ActiveTransactionsLockHandle,
) {
    drop(handle.0.take())
}

#[no_mangle]
pub extern "C" fn rsn_active_transactions_lock_owns_lock(
    handle: &ActiveTransactionsLockHandle,
) -> bool {
    handle.0.is_some()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_active_transactions_lock_stopped(
    handle: &ActiveTransactionsLockHandle,
) -> bool {
    handle.0.as_ref().unwrap().stopped
}

#[no_mangle]
pub unsafe extern "C" fn rsn_active_transactions_lock_stop(
    handle: &mut ActiveTransactionsLockHandle,
) {
    handle.0.as_mut().unwrap().stopped = true;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_active_transactions_lock_roots_size(
    handle: &ActiveTransactionsLockHandle,
) -> usize {
    handle.0.as_ref().unwrap().roots.len()
}

#[no_mangle]
pub extern "C" fn rsn_active_transactions_lock_roots_clear(
    handle: &mut ActiveTransactionsLockHandle,
) {
    handle.0.as_mut().unwrap().roots.clear();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_active_transactions_lock_roots_insert(
    handle: &mut ActiveTransactionsLockHandle,
    root: *const u8,
    previous: *const u8,
    election: &ElectionHandle,
) {
    let root = QualifiedRoot::new(Root::from_ptr(root), BlockHash::from_ptr(previous));
    handle
        .0
        .as_mut()
        .unwrap()
        .roots
        .insert(root, Arc::clone(election));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_active_transactions_lock_roots_erase(
    handle: &mut ActiveTransactionsLockHandle,
    root: *const u8,
    previous: *const u8,
) {
    let root = QualifiedRoot::new(Root::from_ptr(root), BlockHash::from_ptr(previous));
    handle.0.as_mut().unwrap().roots.erase(&root)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_active_transactions_lock_roots_exists(
    handle: &ActiveTransactionsLockHandle,
    root: *const u8,
    previous: *const u8,
) -> bool {
    let root = QualifiedRoot::new(Root::from_ptr(root), BlockHash::from_ptr(previous));
    handle.0.as_ref().unwrap().roots.get(&root).is_some()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_active_transactions_lock_roots_find(
    handle: &ActiveTransactionsLockHandle,
    root: *const u8,
    previous: *const u8,
) -> *mut ElectionHandle {
    let root = QualifiedRoot::new(Root::from_ptr(root), BlockHash::from_ptr(previous));
    match handle.0.as_ref().unwrap().roots.get(&root) {
        Some(election) => Box::into_raw(Box::new(ElectionHandle(Arc::clone(election)))),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn rsn_active_transactions_lock_count_by_behavior(
    handle: &ActiveTransactionsLockHandle,
    behavior: u8,
) -> u64 {
    handle
        .0
        .as_ref()
        .unwrap()
        .count_by_behavior(FromPrimitive::from_u8(behavior).unwrap())
}

#[no_mangle]
pub extern "C" fn rsn_active_transactions_lock_count_by_behavior_inc(
    handle: &mut ActiveTransactionsLockHandle,
    behavior: u8,
) {
    let count = handle
        .0
        .as_mut()
        .unwrap()
        .count_by_behavior_mut(FromPrimitive::from_u8(behavior).unwrap());
    *count += 1;
}

#[no_mangle]
pub extern "C" fn rsn_active_transactions_lock_count_by_behavior_dec(
    handle: &mut ActiveTransactionsLockHandle,
    behavior: u8,
) {
    let count = handle
        .0
        .as_mut()
        .unwrap()
        .count_by_behavior_mut(FromPrimitive::from_u8(behavior).unwrap());
    *count -= 1;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_active_transactions_lock_blocks_insert(
    handle: &mut ActiveTransactionsLockHandle,
    hash: *const u8,
    election: &ElectionHandle,
) {
    let hash = BlockHash::from_ptr(hash);
    handle
        .0
        .as_mut()
        .unwrap()
        .blocks
        .insert(hash, Arc::clone(election));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_active_transactions_lock_blocks_erase(
    handle: &mut ActiveTransactionsLockHandle,
    hash: *const u8,
) -> bool {
    let hash = BlockHash::from_ptr(hash);
    handle.0.as_mut().unwrap().blocks.remove(&hash).is_some()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_active_transactions_lock_blocks_len(
    handle: &ActiveTransactionsLockHandle,
) -> usize {
    handle.0.as_ref().unwrap().blocks.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_active_transactions_lock_blocks_clear(
    handle: &mut ActiveTransactionsLockHandle,
) {
    handle.0.as_mut().unwrap().blocks.clear();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_active_transactions_lock_blocks_find(
    handle: &ActiveTransactionsLockHandle,
    hash: *const u8,
) -> *mut ElectionHandle {
    let hash = BlockHash::from_ptr(hash);
    match handle.0.as_ref().unwrap().blocks.get(&hash) {
        Some(election) => Box::into_raw(Box::new(ElectionHandle(Arc::clone(election)))),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_active_transactions_tally_impl(
    handle: &ActiveTransactionsHandle,
    lock_handle: &mut ElectionLockHandle,
) -> *mut TallyBlocksHandle {
    let tally = handle.tally_impl(lock_handle.0.as_mut().unwrap());
    Box::into_raw(Box::new(TallyBlocksHandle(
        tally
            .iter()
            .map(|(key, value)| (key.deref().clone(), Arc::clone(value)))
            .collect(),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_active_transactions_have_quorum(
    handle: &ActiveTransactionsHandle,
    tally: &TallyBlocksHandle,
) -> bool {
    let ordered_tally: BTreeMap<TallyKey, Arc<BlockEnum>> = tally
        .0
        .iter()
        .map(|(k, v)| (TallyKey(*k), Arc::clone(v)))
        .collect();
    handle.have_quorum(&ordered_tally)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_active_transactions_remove_votes(
    handle: &ActiveTransactionsHandle,
    election: &ElectionHandle,
    lock_handle: &mut ElectionLockHandle,
    hash: *const u8,
) {
    handle.remove_votes(
        election,
        lock_handle.0.as_mut().unwrap(),
        &BlockHash::from_ptr(hash),
    );
}

pub struct TallyBlocksHandle(Vec<(Amount, Arc<BlockEnum>)>);

#[no_mangle]
pub unsafe extern "C" fn rsn_tally_blocks_create() -> *mut TallyBlocksHandle {
    Box::into_raw(Box::new(TallyBlocksHandle(Vec::new())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tally_blocks_destroy(handle: *mut TallyBlocksHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_tally_blocks_len(handle: &TallyBlocksHandle) -> usize {
    handle.0.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tally_blocks_get(
    handle: &TallyBlocksHandle,
    index: usize,
    weight: *mut u8,
) -> *mut BlockHandle {
    let (amount, block) = handle.0.get(index).unwrap();
    amount.copy_bytes(weight);
    BlockHandle::new(Arc::clone(block))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_tally_blocks_insert(
    handle: &mut TallyBlocksHandle,
    weight: *const u8,
    block: &BlockHandle,
) {
    handle.0.push((Amount::from_ptr(weight), Arc::clone(block)))
}

#[no_mangle]
pub extern "C" fn rsn_active_transactions_lock_roots_get_elections(
    handle: &ActiveTransactionsLockHandle,
) -> *mut ElectionVecHandle {
    let elections: Vec<_> = handle
        .0
        .as_ref()
        .unwrap()
        .roots
        .iter_sequenced()
        .map(|(_, election)| Arc::clone(election))
        .collect();

    Box::into_raw(Box::new(ElectionVecHandle(elections)))
}

pub struct ElectionVecHandle(Vec<Arc<Election>>);

#[no_mangle]
pub unsafe extern "C" fn rsn_election_vec_destroy(handle: *mut ElectionVecHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_election_vec_len(handle: &ElectionVecHandle) -> usize {
    handle.0.len()
}

#[no_mangle]
pub extern "C" fn rsn_election_vec_get(
    handle: &ElectionVecHandle,
    index: usize,
) -> *mut ElectionHandle {
    Box::into_raw(Box::new(ElectionHandle(Arc::clone(&handle.0[index]))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_active_transactions_cooldown_time_s(
    handle: &ActiveTransactionsHandle,
    weight: *const u8,
) -> u64 {
    handle.cooldown_time(Amount::from_ptr(weight)).as_secs()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_active_transactions_remove_election_winner_details(
    handle: &ActiveTransactionsHandle,
    hash: *const u8,
) -> *mut ElectionHandle {
    match handle.remove_election_winner_details(&BlockHash::from_ptr(hash)) {
        Some(election) => ElectionHandle::new(election),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_active_transactions_confirm_once(
    handle: &ActiveTransactionsHandle,
    election_lock: &mut ElectionLockHandle,
    election: &ElectionHandle,
) {
    handle.confirm_once(election_lock.take().unwrap(), Arc::clone(election));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_active_transactions_process_confirmed(
    handle: &ActiveTransactionsHandle,
    status: &ElectionStatusHandle,
    iteration: u64,
) {
    handle.process_confirmed(status.0.clone(), iteration);
}

pub struct ElectionWinnerDetailsLock(
    Option<MutexGuard<'static, HashMap<BlockHash, Arc<Election>>>>,
);

#[no_mangle]
pub unsafe extern "C" fn rsn_active_transactions_lock_election_winner_details(
    handle: &ActiveTransactionsHandle,
) -> *mut ElectionWinnerDetailsLock {
    let guard = handle.election_winner_details.lock().unwrap();
    let guard = std::mem::transmute::<
        MutexGuard<HashMap<BlockHash, Arc<Election>>>,
        MutexGuard<'static, HashMap<BlockHash, Arc<Election>>>,
    >(guard);
    Box::into_raw(Box::new(ElectionWinnerDetailsLock(Some(guard))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_winner_details_lock_destroy(
    handle: *mut ElectionWinnerDetailsLock,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_winner_details_lock_unlock(
    handle: &mut ElectionWinnerDetailsLock,
) {
    drop(handle.0.take())
}

#[no_mangle]
pub extern "C" fn rsn_election_winner_details_len(handle: &ElectionWinnerDetailsLock) -> usize {
    handle.0.as_ref().unwrap().len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_winner_details_contains(
    handle: &ElectionWinnerDetailsLock,
    hash: *const u8,
) -> bool {
    handle
        .0
        .as_ref()
        .unwrap()
        .contains_key(&BlockHash::from_ptr(hash))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_winner_details_insert(
    handle: &mut ElectionWinnerDetailsLock,
    hash: *const u8,
    election: &ElectionHandle,
) {
    handle
        .0
        .as_mut()
        .unwrap()
        .insert(BlockHash::from_ptr(hash), Arc::clone(election));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_winner_details_remove(
    handle: &mut ElectionWinnerDetailsLock,
    hash: *const u8,
) {
    handle
        .0
        .as_mut()
        .unwrap()
        .remove(&BlockHash::from_ptr(hash));
}
