use crate::ledger::datastore::LedgerHandle;
use rsnano_core::{Account, Amount};
use rsnano_node::wallets::WalletRepresentatives;
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

pub struct WalletRepresentativesHandle(WalletRepresentatives);

impl Deref for WalletRepresentativesHandle {
    type Target = WalletRepresentatives;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for WalletRepresentativesHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_wallet_representatives_create(
    vote_minimum: *const u8,
    ledger: &LedgerHandle,
) -> *mut WalletRepresentativesHandle {
    Box::into_raw(Box::new(WalletRepresentativesHandle(
        WalletRepresentatives::new(Amount::from_ptr(vote_minimum), Arc::clone(ledger)),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_wallet_representatives_destroy(
    handle: *mut WalletRepresentativesHandle,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_wallet_representatives_have_half_rep(
    handle: &WalletRepresentativesHandle,
) -> bool {
    handle.have_half_rep()
}

#[no_mangle]
pub extern "C" fn rsn_wallet_representatives_voting_reps(
    handle: &WalletRepresentativesHandle,
) -> u64 {
    handle.voting_reps()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_wallet_representatives_exists(
    handle: &WalletRepresentativesHandle,
    rep: *const u8,
) -> bool {
    handle.exists(&Account::from_ptr(rep))
}

#[no_mangle]
pub extern "C" fn rsn_wallet_representatives_clear(handle: &mut WalletRepresentativesHandle) {
    handle.clear();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_wallet_representatives_check_rep(
    handle: &mut WalletRepresentativesHandle,
    account: *const u8,
    half_principal_weight: *const u8,
) -> bool {
    handle.check_rep(
        Account::from_ptr(account),
        Amount::from_ptr(half_principal_weight),
    )
}
