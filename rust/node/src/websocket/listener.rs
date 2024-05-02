use crate::wallets::Wallets;

use super::{Message, Topic};
use anyhow::Result;
use rsnano_core::{utils::PropertyTreeReader, Account};
use std::{
    collections::{HashMap, HashSet},
    ffi::c_void,
    sync::{Arc, Mutex},
};
use tracing::warn;

pub trait Listener: Send + Sync {
    fn broadcast(&self, message: &Message) -> Result<()>;
}

pub struct NullListener {}

impl NullListener {
    pub fn new() -> Self {
        Self {}
    }
}

impl Listener for NullListener {
    fn broadcast(&self, _message: &Message) -> Result<()> {
        Ok(())
    }
}

pub enum Options {
    Confirmation(ConfirmationOptions),
    Vote(VoteOptions),
    Other,
}

pub struct ConfirmationOptions {
    pub include_election_info: bool,
    pub include_election_info_with_votes: bool,
    pub include_sideband_info: bool,
    pub include_block: bool,
    pub has_account_filtering_options: bool,
    pub all_local_accounts: bool,
    pub confirmation_types: u8,
    pub accounts: HashSet<String>,
    wallets: Arc<Wallets>,
}

impl ConfirmationOptions {
    const TYPE_ACTIVE_QUORUM: u8 = 1;
    const TYPE_ACTIVE_CONFIRMATION_HEIGHT: u8 = 2;
    const TYPE_INACTIVE: u8 = 4;
    const TYPE_ALL_ACTIVE: u8 = Self::TYPE_ACTIVE_QUORUM | Self::TYPE_ACTIVE_CONFIRMATION_HEIGHT;
    const TYPE_ALL: u8 = Self::TYPE_ALL_ACTIVE | Self::TYPE_INACTIVE;

    pub fn new(wallets: Arc<Wallets>) -> Self {
        Self {
            include_election_info: false,
            include_election_info_with_votes: false,
            include_sideband_info: false,
            include_block: true,
            has_account_filtering_options: false,
            all_local_accounts: false,
            confirmation_types: Self::TYPE_ALL,
            accounts: HashSet::new(),
            wallets,
        }
    }

    /**
     * Update some existing options
     * Filtering options:
     * - "accounts_add" (array of std::strings) - additional accounts for which blocks should not be filtered
     * - "accounts_del" (array of std::strings) - accounts for which blocks should be filtered
     * @return false
     */
    pub fn update(&mut self, options: impl PropertyTreeReader) -> bool {
        let mut update_accounts = |accounts_text: &dyn PropertyTreeReader, insert: bool| {
            self.has_account_filtering_options = true;
            for account in accounts_text.get_children() {
                match Account::decode_account(account.1.data()) {
                    Ok(result) => {
                        // Re-encode to keep old prefix support
                        let encoded = result.encode_account();
                        if insert {
                            self.accounts.insert(encoded);
                        } else {
                            self.accounts.remove(&encoded);
                        }
                    }
                    Err(_) => {
                        warn!(
                            "Invalid account provided for filtering blocks: {}",
                            account.1.data()
                        );
                    }
                }
            }
        };

        // Adding accounts as filter exceptions
        if let Some(accounts_add) = options.get_child("accounts_add") {
            update_accounts(&*accounts_add, true);
        }

        // Removing accounts as filter exceptions
        if let Some(accounts_del) = options.get_child("accounts_del") {
            update_accounts(&*accounts_del, false);
        }

        self.check_filter_empty();
        false
    }

    pub fn check_filter_empty(&self) {
        // Warn the user if the options resulted in an empty filter
        if self.has_account_filtering_options
            && !self.all_local_accounts
            && self.accounts.is_empty()
        {
            warn!("Provided options resulted in an empty account confirmation filter");
        }
    }
}

pub struct VoteOptions {}

impl VoteOptions {
    pub fn new() -> Self {
        Self {}
    }
}

pub struct WebsocketListener {
    cpp_pointer: *mut c_void,
    subscriptions: Mutex<HashMap<Topic, Options>>,
}

impl WebsocketListener {
    pub fn new(cpp_pointer: *mut c_void) -> Self {
        Self {
            cpp_pointer,
            subscriptions: Mutex::new(HashMap::new()),
        }
    }
}

unsafe impl Send for WebsocketListener {}
unsafe impl Sync for WebsocketListener {}

pub type BroadcastCallback = fn(*mut c_void, &Message) -> Result<()>;
pub static mut BROADCAST_CALLBACK: Option<BroadcastCallback> = None;

impl Listener for WebsocketListener {
    fn broadcast(&self, message: &Message) -> Result<()> {
        unsafe {
            match BROADCAST_CALLBACK {
                Some(f) => f(self.cpp_pointer, message),
                None => Err(anyhow!("BROADCAST_CALLBACK missing")),
            }
        }
    }
}
