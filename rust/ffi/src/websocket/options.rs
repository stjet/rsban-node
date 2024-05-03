use crate::{to_rust_string, wallets::LmdbWalletsHandle, FfiPropertyTree};
use rsnano_node::websocket::{ConfirmationOptions, Options, VoteOptions};
use std::{
    ffi::{c_char, c_void},
    ops::{Deref, DerefMut},
    sync::Arc,
};

use super::MessageDto;

pub struct WebsocketOptionsHandle(Options);

impl WebsocketOptionsHandle {
    pub fn new(options: Options) -> *mut Self {
        Box::into_raw(Box::new(Self(options)))
    }

    pub fn confirmation_options(&self) -> &ConfirmationOptions {
        let Options::Confirmation(options) = &self.0 else {
            panic!("not of type ConfirmationOptions")
        };
        options
    }

    pub fn confirmation_options_mut(&mut self) -> &mut ConfirmationOptions {
        let Options::Confirmation(options) = &mut self.0 else {
            panic!("not of type ConfirmationOptions")
        };
        options
    }
}

impl Deref for WebsocketOptionsHandle {
    type Target = Options;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for WebsocketOptionsHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_websocket_options_create() -> *mut WebsocketOptionsHandle {
    WebsocketOptionsHandle::new(Options::Other)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_websocket_options_destroy(handle: *mut WebsocketOptionsHandle) {
    drop(Box::from_raw(handle))
}

/*
 * ConfirmationOptions
 */

#[no_mangle]
pub extern "C" fn rsn_confirmation_options_create(
    wallets: &LmdbWalletsHandle,
) -> *mut WebsocketOptionsHandle {
    WebsocketOptionsHandle::new(Options::Confirmation(ConfirmationOptions::new(Arc::clone(
        wallets,
    ))))
}

#[no_mangle]
pub extern "C" fn rsn_confirmation_options_include_election_info(
    handle: &WebsocketOptionsHandle,
) -> bool {
    handle.confirmation_options().include_election_info
}

#[no_mangle]
pub extern "C" fn rsn_confirmation_options_include_election_info_set(
    handle: &mut WebsocketOptionsHandle,
    value: bool,
) {
    handle.confirmation_options_mut().include_election_info = value;
}

#[no_mangle]
pub extern "C" fn rsn_confirmation_options_include_election_info_with_votes(
    handle: &WebsocketOptionsHandle,
) -> bool {
    handle
        .confirmation_options()
        .include_election_info_with_votes
}

#[no_mangle]
pub extern "C" fn rsn_confirmation_options_include_election_info_with_votes_set(
    handle: &mut WebsocketOptionsHandle,
    value: bool,
) {
    handle
        .confirmation_options_mut()
        .include_election_info_with_votes = value;
}

#[no_mangle]
pub extern "C" fn rsn_confirmation_options_include_sideband_info(
    handle: &WebsocketOptionsHandle,
) -> bool {
    handle.confirmation_options().include_sideband_info
}

#[no_mangle]
pub extern "C" fn rsn_confirmation_options_include_sideband_info_set(
    handle: &mut WebsocketOptionsHandle,
    value: bool,
) {
    handle.confirmation_options_mut().include_sideband_info = value;
}

#[no_mangle]
pub extern "C" fn rsn_confirmation_options_include_block(handle: &WebsocketOptionsHandle) -> bool {
    handle.confirmation_options().include_block
}

#[no_mangle]
pub extern "C" fn rsn_confirmation_options_include_block_set(
    handle: &mut WebsocketOptionsHandle,
    value: bool,
) {
    handle.confirmation_options_mut().include_block = value;
}

#[no_mangle]
pub extern "C" fn rsn_confirmation_options_has_account_filtering_options(
    handle: &WebsocketOptionsHandle,
) -> bool {
    handle.confirmation_options().has_account_filtering_options
}

#[no_mangle]
pub extern "C" fn rsn_confirmation_options_has_account_filtering_options_set(
    handle: &mut WebsocketOptionsHandle,
    value: bool,
) {
    handle
        .confirmation_options_mut()
        .has_account_filtering_options = value;
}

#[no_mangle]
pub extern "C" fn rsn_confirmation_options_all_local_accounts(
    handle: &WebsocketOptionsHandle,
) -> bool {
    handle.confirmation_options().all_local_accounts
}

#[no_mangle]
pub extern "C" fn rsn_confirmation_options_all_local_accounts_set(
    handle: &mut WebsocketOptionsHandle,
    value: bool,
) {
    handle.confirmation_options_mut().all_local_accounts = value;
}

#[no_mangle]
pub extern "C" fn rsn_confirmation_options_confirmation_types(
    handle: &WebsocketOptionsHandle,
) -> u8 {
    handle.confirmation_options().confirmation_types
}

#[no_mangle]
pub extern "C" fn rsn_confirmation_options_confirmation_types_set(
    handle: &mut WebsocketOptionsHandle,
    value: u8,
) {
    handle.confirmation_options_mut().confirmation_types = value;
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_options_accounts_insert(
    handle: &mut WebsocketOptionsHandle,
    account: *const c_char,
) {
    handle
        .confirmation_options_mut()
        .accounts
        .insert(to_rust_string(account));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_options_accounts_contains(
    handle: &mut WebsocketOptionsHandle,
    account: *const c_char,
) -> bool {
    handle
        .confirmation_options_mut()
        .accounts
        .contains(&to_rust_string(account))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_options_accounts_remove(
    handle: &mut WebsocketOptionsHandle,
    account: *const c_char,
) {
    handle
        .confirmation_options_mut()
        .accounts
        .remove(&to_rust_string(account));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_options_accounts_is_empty(
    handle: &mut WebsocketOptionsHandle,
) -> bool {
    handle.confirmation_options_mut().accounts.is_empty()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_options_should_filter(
    handle: &WebsocketOptionsHandle,
    message: &MessageDto,
) -> bool {
    handle.confirmation_options().should_filter(&message.into())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirmation_options_update(
    handle: &mut WebsocketOptionsHandle,
    options: *mut c_void,
) -> bool {
    handle
        .confirmation_options_mut()
        .update(FfiPropertyTree::new_borrowed(options))
}

/*
 * VoteOptions
 */

#[no_mangle]
pub extern "C" fn rsn_vote_options_create() -> *mut WebsocketOptionsHandle {
    WebsocketOptionsHandle::new(Options::Vote(VoteOptions::new()))
}
