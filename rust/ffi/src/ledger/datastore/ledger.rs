use super::lmdb::{LmdbStoreHandle, TransactionHandle};
use crate::{
    core::{copy_block_array_dto, AccountInfoHandle, BlockArrayDto, BlockHandle},
    ledger::{GenerateCacheHandle, LedgerCacheHandle, LedgerConstantsDto},
    StatHandle, StringDto,
};
use num_traits::FromPrimitive;
use rsnano_core::{Account, Amount, BlockEnum, BlockHash, Epoch, Link, QualifiedRoot};
use rsnano_ledger::{BlockStatus, Ledger};
use rsnano_node::stats::LedgerStats;
use std::{ops::Deref, ptr::null_mut, sync::Arc};

pub struct LedgerHandle(pub Arc<Ledger>);

impl Deref for LedgerHandle {
    type Target = Arc<Ledger>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_create(
    store: *mut LmdbStoreHandle,
    constants: *const LedgerConstantsDto,
    stats: *mut StatHandle,
    generate_cache: *mut GenerateCacheHandle,
) -> *mut LedgerHandle {
    let stats = (*stats).deref().to_owned();
    let mut ledger = Ledger::with_cache(
        (*store).deref().to_owned(),
        (&*constants).try_into().unwrap(),
        &*generate_cache,
    )
    .unwrap();

    ledger.set_observer(Arc::new(LedgerStats::new(stats)));

    Box::into_raw(Box::new(LedgerHandle(Arc::new(ledger))))
}

#[no_mangle]
pub extern "C" fn rsn_ledger_destroy(handle: *mut LedgerHandle) {
    drop(unsafe { Box::from_raw(handle) });
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_pruning_enabled(handle: *mut LedgerHandle) -> bool {
    (*handle).0.pruning_enabled()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_enable_pruning(handle: *mut LedgerHandle) {
    (*handle).0.enable_pruning()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_bootstrap_weight_max_blocks(handle: *mut LedgerHandle) -> u64 {
    (*handle).0.bootstrap_weight_max_blocks()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_set_bootstrap_weight_max_blocks(
    handle: *mut LedgerHandle,
    max: u64,
) {
    (*handle).0.set_bootstrap_weight_max_blocks(max)
}

#[repr(C)]
pub struct BootstrapWeightsItem {
    pub account: [u8; 32],
    pub weight: [u8; 16],
}

pub struct BootstrapWeightsRawPtr(Vec<BootstrapWeightsItem>);

#[repr(C)]
pub struct BootstrapWeightsDto {
    pub accounts: *const BootstrapWeightsItem,
    pub count: usize,
    pub raw_ptr: *mut BootstrapWeightsRawPtr,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_bootstrap_weights(
    handle: *mut LedgerHandle,
    result: *mut BootstrapWeightsDto,
) {
    let weights = (*handle).0.bootstrap_weights.lock().unwrap().to_owned();
    let items = weights
        .iter()
        .map(|(k, v)| BootstrapWeightsItem {
            account: *k.as_bytes(),
            weight: v.to_be_bytes(),
        })
        .collect();
    let raw_ptr = Box::new(BootstrapWeightsRawPtr(items));

    (*result).count = raw_ptr.0.len();
    (*result).accounts = raw_ptr.0.as_ptr();
    (*result).raw_ptr = Box::into_raw(raw_ptr);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_destroy_bootstrap_weights_dto(dto: *mut BootstrapWeightsDto) {
    drop(Box::from_raw((*dto).raw_ptr))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_set_bootstrap_weights(
    handle: *mut LedgerHandle,
    accounts: *const BootstrapWeightsItem,
    count: usize,
) {
    let dtos = std::slice::from_raw_parts(accounts, count);
    let weights = dtos
        .iter()
        .map(|d| {
            (
                Account::from_bytes(d.account),
                Amount::from_be_bytes(d.weight),
            )
        })
        .collect();
    *(*handle).0.bootstrap_weights.lock().unwrap() = weights;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_get_cache_handle(
    handle: *mut LedgerHandle,
) -> *mut LedgerCacheHandle {
    LedgerCacheHandle::new((*handle).0.cache.clone())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_balance(
    handle: &LedgerHandle,
    txn: &TransactionHandle,
    hash: *const u8,
    result: *mut u8,
) -> bool {
    if let Some(balance) = handle.balance(txn.as_txn(), &BlockHash::from_ptr(hash)) {
        balance.copy_bytes(result);
        true
    } else {
        false
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_account_balance(
    handle: *mut LedgerHandle,
    txn: *mut TransactionHandle,
    account: *const u8,
    only_confirmed: bool,
    result: *mut u8,
) {
    let balance =
        (*handle)
            .0
            .account_balance((*txn).as_txn(), &Account::from_ptr(account), only_confirmed);
    balance.copy_bytes(result);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_account_receivable(
    handle: *mut LedgerHandle,
    txn: *mut TransactionHandle,
    account: *const u8,
    only_confirmed: bool,
    result: *mut u8,
) {
    let balance = (*handle).0.account_receivable(
        (*txn).as_txn(),
        &Account::from_ptr(account),
        only_confirmed,
    );
    balance.copy_bytes(result);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_block_confirmed(
    handle: *mut LedgerHandle,
    txn: *mut TransactionHandle,
    hash: *const u8,
) -> bool {
    (*handle)
        .0
        .block_confirmed((*txn).as_txn(), &BlockHash::from_ptr(hash))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_block_or_pruned_exists(
    handle: *mut LedgerHandle,
    hash: *const u8,
) -> bool {
    (*handle)
        .0
        .block_or_pruned_exists(&BlockHash::from_ptr(hash))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_block_or_pruned_exists_txn(
    handle: *mut LedgerHandle,
    txn: *mut TransactionHandle,
    hash: *const u8,
) -> bool {
    (*handle)
        .0
        .block_or_pruned_exists_txn((*txn).as_txn(), &BlockHash::from_ptr(hash))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_block_text(
    handle: *mut LedgerHandle,
    hash: *const u8,
    result: *mut StringDto,
) {
    *result = match (*handle).0.block_text(&BlockHash::from_ptr(hash)) {
        Ok(s) => s.into(),
        Err(_) => "".into(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_hash_root_random(
    handle: *mut LedgerHandle,
    txn: *mut TransactionHandle,
    result_hash: *mut u8,
    result_root: *mut u8,
) {
    let (hash, root) = (*handle)
        .0
        .hash_root_random((*txn).as_txn())
        .unwrap_or_default();
    hash.copy_bytes(result_hash);
    root.copy_bytes(result_root);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_weight(
    handle: *mut LedgerHandle,
    account: *const u8,
    result: *mut u8,
) {
    let weight = (*handle).0.weight(&Account::from_ptr(account));
    weight.copy_bytes(result);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_account(
    handle: &LedgerHandle,
    txn: &TransactionHandle,
    hash: *const u8,
    result: *mut u8,
) -> bool {
    match handle.account(txn.as_txn(), &BlockHash::from_ptr(hash)) {
        Some(account) => {
            account.copy_bytes(result);
            true
        }
        None => false,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_version(
    handle: &LedgerHandle,
    txn: &mut TransactionHandle,
    hash: *const u8,
) -> u8 {
    handle.0.version(txn.as_txn(), &BlockHash::from_ptr(hash)) as u8
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_account_height(
    handle: &LedgerHandle,
    txn: &mut TransactionHandle,
    hash: *const u8,
) -> u64 {
    handle
        .0
        .account_height(txn.as_txn(), &BlockHash::from_ptr(hash))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_amount(
    handle: &LedgerHandle,
    txn: &TransactionHandle,
    hash: *const u8,
    result: *mut u8,
) -> bool {
    match handle.amount(txn.as_txn(), &BlockHash::from_ptr(hash)) {
        Some(amount) => {
            amount.copy_bytes(result);
            true
        }
        None => false,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_latest(
    handle: *mut LedgerHandle,
    txn: *mut TransactionHandle,
    account: *const u8,
    result: *mut u8,
) {
    let latest = (*handle)
        .0
        .latest((*txn).as_txn(), &Account::from_ptr(account))
        .unwrap_or_default();
    latest.copy_bytes(result);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_latest_root(
    handle: *mut LedgerHandle,
    txn: *mut TransactionHandle,
    account: *const u8,
    result: *mut u8,
) {
    let latest = (*handle)
        .0
        .latest_root((*txn).as_txn(), &Account::from_ptr(account));
    latest.copy_bytes(result);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_is_epoch_link(
    handle: *mut LedgerHandle,
    link: *const u8,
) -> bool {
    (*handle).0.is_epoch_link(&Link::from_ptr(link))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_find_receive_block_by_send_hash(
    handle: *mut LedgerHandle,
    txn: *mut TransactionHandle,
    destination: *const u8,
    send_block_hash: *const u8,
) -> *mut BlockHandle {
    let block = (*handle).0.find_receive_block_by_send_hash(
        (*txn).as_txn(),
        &Account::from_ptr(destination),
        &BlockHash::from_ptr(send_block_hash),
    );
    match block {
        Some(b) => Box::into_raw(Box::new(BlockHandle(Arc::new(b)))),
        None => null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_epoch_signer(
    handle: *mut LedgerHandle,
    link: *const u8,
    result: *mut u8,
) {
    let signer = (*handle)
        .0
        .constants
        .epochs
        .epoch_signer(&Link::from_ptr(link))
        .unwrap_or_default();
    signer.copy_bytes(result);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_epoch_link(
    handle: *mut LedgerHandle,
    epoch: u8,
    result: *mut u8,
) {
    let link = (*handle)
        .0
        .epoch_link(Epoch::from_u8(epoch).unwrap())
        .unwrap_or_default();
    link.copy_bytes(result);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_update_account(
    handle: *mut LedgerHandle,
    txn: *mut TransactionHandle,
    account: *const u8,
    old_info: *const AccountInfoHandle,
    new_info: *const AccountInfoHandle,
) {
    (*handle).0.update_account(
        (*txn).as_write_txn(),
        &Account::from_ptr(account),
        &*old_info,
        &*new_info,
    );
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_pruning_action(
    handle: *mut LedgerHandle,
    txn: *mut TransactionHandle,
    hash: *const u8,
    batch_size: u64,
) -> u64 {
    (*handle).0.pruning_action(
        (*txn).as_write_txn(),
        &BlockHash::from_ptr(hash),
        batch_size,
    )
}

#[repr(C)]
pub struct UncementedInfoDto {
    pub cemented_frontier: [u8; 32],
    pub frontier: [u8; 32],
    pub account: [u8; 32],
}

#[repr(C)]
pub struct UnconfirmedFrontierDto {
    pub height_delta: u64,
    pub info: UncementedInfoDto,
}

pub struct UnconfirmedFrontiersHandle(Vec<UnconfirmedFrontierDto>);

#[repr(C)]
pub struct UnconfirmedFrontierArrayDto {
    pub items: *const UnconfirmedFrontierDto,
    pub count: usize,
    pub raw_ptr: *mut UnconfirmedFrontiersHandle,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_unconfirmed_frontiers(
    handle: *mut LedgerHandle,
    result: *mut UnconfirmedFrontierArrayDto,
) {
    let unconfirmed = (*handle).0.unconfirmed_frontiers();
    let handle = Box::new(UnconfirmedFrontiersHandle(
        unconfirmed
            .iter()
            .flat_map(|(&k, v)| {
                v.iter().map(move |info| UnconfirmedFrontierDto {
                    height_delta: k,
                    info: UncementedInfoDto {
                        cemented_frontier: *info.cemented_frontier.as_bytes(),
                        frontier: *info.frontier.as_bytes(),
                        account: *info.account.as_bytes(),
                    },
                })
            })
            .collect(),
    ));

    (*result).items = handle.0.as_ptr();
    (*result).count = handle.0.len();
    (*result).raw_ptr = Box::into_raw(handle);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_unconfirmed_frontiers_destroy(
    result: *mut UnconfirmedFrontierArrayDto,
) {
    drop(Box::from_raw((*result).raw_ptr))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_bootstrap_weight_reached(handle: *mut LedgerHandle) -> bool {
    (*handle).0.bootstrap_weight_reached()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_dependent_blocks(
    handle: *mut LedgerHandle,
    txn: *mut TransactionHandle,
    block: &BlockHandle,
    result1: *mut u8,
    result2: *mut u8,
) {
    let (first, second) = (*handle).0.dependent_blocks((*txn).as_txn(), &block);
    first.copy_bytes(result1);
    second.copy_bytes(result2);
}

#[no_mangle]
pub extern "C" fn rsn_ledger_dependents_confirmed(
    handle: &LedgerHandle,
    txn: &TransactionHandle,
    block: &BlockHandle,
) -> bool {
    handle.0.dependents_confirmed(txn.as_txn(), &block)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_representative(
    handle: &LedgerHandle,
    txn: &TransactionHandle,
    hash: *const u8,
    result: *mut u8,
) {
    let representative = handle
        .0
        .representative_block_hash(txn.as_txn(), &BlockHash::from_ptr(hash));
    representative.copy_bytes(result);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_rollback(
    handle: *mut LedgerHandle,
    txn: *mut TransactionHandle,
    hash: *const u8,
    result: &mut BlockArrayDto,
) -> bool {
    match (*handle)
        .0
        .rollback((*txn).as_write_txn(), &BlockHash::from_ptr(hash))
    {
        Ok(mut block_list) => {
            let block_list = block_list.drain(..).map(|b| Arc::new(b)).collect();
            copy_block_array_dto(block_list, result);
            false
        }
        Err(_) => {
            copy_block_array_dto(Vec::new(), result);
            true
        }
    }
}

#[repr(C)]
pub struct ProcessReturnDto {
    pub code: u8,
}

impl From<BlockStatus> for ProcessReturnDto {
    fn from(result: BlockStatus) -> Self {
        Self { code: result as u8 }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_process(
    handle: &LedgerHandle,
    txn: &mut TransactionHandle,
    block: &mut BlockHandle,
    result: *mut ProcessReturnDto,
) {
    // this is undefined behaviour and should be fixed ASAP:
    let block_ptr = Arc::as_ptr(&block) as *mut BlockEnum;
    let res = handle.0.process(txn.as_write_txn(), &mut *block_ptr);
    let res = match res {
        Ok(()) => BlockStatus::Progress,
        Err(res) => res,
    };
    (*result) = res.into();
}
