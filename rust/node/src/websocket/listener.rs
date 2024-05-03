use super::{Message, Topic};
use crate::wallets::Wallets;
use anyhow::Result;
use rsnano_core::{utils::PropertyTree, Account};
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

    pub fn new(wallets: Arc<Wallets>, options_a: &dyn PropertyTree) -> Self {
        let mut result = Self {
            include_election_info: false,
            include_election_info_with_votes: false,
            include_sideband_info: false,
            include_block: true,
            has_account_filtering_options: false,
            all_local_accounts: false,
            confirmation_types: Self::TYPE_ALL,
            accounts: HashSet::new(),
            wallets,
        };
        // Non-account filtering options
        result.include_block = options_a.get_bool("include_block", true);
        result.include_election_info = options_a.get_bool("include_election_info", false);
        result.include_election_info_with_votes =
            options_a.get_bool("include_election_info_with_votes", false);
        result.include_sideband_info = options_a.get_bool("include_sideband_info", false);

        let type_l = options_a
            .get_string("confirmation_type")
            .unwrap_or_else(|_| "all".to_string());

        if type_l.eq_ignore_ascii_case("active") {
            result.confirmation_types = Self::TYPE_ALL_ACTIVE;
        } else if type_l.eq_ignore_ascii_case("active_quorum") {
            result.confirmation_types = Self::TYPE_ACTIVE_QUORUM;
        } else if type_l.eq_ignore_ascii_case("active_confirmation_height") {
            result.confirmation_types = Self::TYPE_ACTIVE_CONFIRMATION_HEIGHT;
        } else if type_l.eq_ignore_ascii_case("inactive") {
            result.confirmation_types = Self::TYPE_INACTIVE;
        } else {
            result.confirmation_types = Self::TYPE_ALL;
        }

        // Account filtering options
        let all_local_accounts_l = options_a.get_bool("all_local_accounts", false);
        if all_local_accounts_l {
            result.all_local_accounts = true;
            result.has_account_filtering_options = true;
            if !result.include_block {
                warn!("Websocket: Filtering option \"all_local_accounts\" requires that \"include_block\" is set to true to be effective");
            }
        }
        let accounts_l = options_a.get_child("accounts");
        if let Some(accounts_l) = accounts_l {
            result.has_account_filtering_options = true;
            for account_l in accounts_l.get_children() {
                match Account::decode_account(&account_l.1.data()) {
                    Ok(result_l) => {
                        // Do not insert the given raw data to keep old prefix support
                        result.accounts.insert(result_l.encode_account());
                    }
                    Err(_) => {
                        warn!(
                            "Invalid account provided for filtering blocks: {}",
                            account_l.1.data()
                        );
                    }
                }
            }

            if !result.include_block {
                warn!("Filtering option \"accounts\" requires that \"include_block\" is set to true to be effective");
            }
        }
        result.check_filter_empty();

        result
    }

    pub fn should_filter(&self, message_a: &Message) -> bool {
        let mut should_filter_conf_type = true;

        let type_text = message_a
            .contents
            .get_string("message.confirmation_type")
            .unwrap_or_default();
        let confirmation_types = self.confirmation_types;
        if type_text == "active_quorum" && (confirmation_types & Self::TYPE_ACTIVE_QUORUM) > 0 {
            should_filter_conf_type = false;
        } else if type_text == "active_confirmation_height"
            && (confirmation_types & Self::TYPE_ACTIVE_CONFIRMATION_HEIGHT) > 0
        {
            should_filter_conf_type = false;
        } else if type_text == "inactive" && (confirmation_types & Self::TYPE_INACTIVE) > 0 {
            should_filter_conf_type = false;
        }

        let mut should_filter_account = self.has_account_filtering_options;
        let destination_text = message_a
            .contents
            .get_string("message.block.link_as_account");
        if let Ok(destination_text) = destination_text {
            let source_text = message_a
                .contents
                .get_string("message.account")
                .unwrap_or_default();
            if self.all_local_accounts {
                let source = Account::decode_account(&source_text).unwrap_or_default();
                let destination = Account::decode_account(&destination_text).unwrap_or_default();
                if self.wallets.exists(&source) || self.wallets.exists(&destination) {
                    should_filter_account = false;
                }
            }
            if self.accounts.contains(&source_text) || self.accounts.contains(&destination_text) {
                should_filter_account = false;
            }
        }

        should_filter_conf_type || should_filter_account
    }

    /**
     * Update some existing options
     * Filtering options:
     * - "accounts_add" (array of std::strings) - additional accounts for which blocks should not be filtered
     * - "accounts_del" (array of std::strings) - accounts for which blocks should be filtered
     * @return false
     */
    pub fn update(&mut self, options: impl PropertyTree) -> bool {
        let mut update_accounts = |accounts_text: &dyn PropertyTree, insert: bool| {
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
