use clap::{Parser, Subcommand};
use commands::{
    accounts::{
        account_create::AccountCreateOptions, account_get::AccountGetOptions,
        account_key::AccountKeyOptions,
    },
    clear::{
        clear_send_ids::ClearSendIdsOptions,
        confirmation_height_clear::ConfirmationHeightClearOptions,
        final_vote_clear::FinalVoteClearOptions, online_weight_clear::OnlineWeightClearOptions,
        peer_clear::PeerClearOptions,
    },
    keys::{key_create::KeyCreateOptions, key_expand::KeyExpandOptions},
    node::{daemon::DaemonOptions, diagnostics::DiagnosticsOptions, initialize::InitializeOptions},
    wallets::{
        wallet_add_adhoc::WalletAddAdhocOptions, wallet_change_seed::WalletChangeSeedOptions,
        wallet_create::WalletCreateOptions, wallet_decrypt_unsafe::WalletDecryptUnsafeOptions,
        wallet_destroy::WalletDestroyOptions, wallet_list::WalletListOptions,
        wallet_remove::WalletRemoveOptions,
        wallet_representative_get::WalletRepresentativeGetOptions,
        wallet_representative_set::WalletRepresentativeSetOptions,
    },
};
use rsnano_core::Networks;
use rsnano_node::{config::NetworkConstants, working_path};
use std::{path::PathBuf, str::FromStr};

mod commands;

#[derive(Parser)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Start node daemon
    Daemon(DaemonOptions),
    /// Initialize the data folder, if it is not already initialised. This command is meant to be run when the data folder is empty, to populate it with the genesis block.
    Initialize(InitializeOptions),
    /// Generates a adhoc random keypair and prints it to stdout
    KeyCreate(KeyCreateOptions),
    /// Derive public key and account number from <key>
    KeyExpand(KeyExpandOptions),
    /// Either specify a single --root to clear or --all to clear all final votes (not recommended)
    FinalVoteClear(FinalVoteClearOptions),
    /// Insert next deterministic key into <wallet>
    AccountCreate(AccountCreateOptions),
    /// Get account number for the <key>
    AccountGet(AccountGetOptions),
    /// Get the public key for <account>
    AccountKey(AccountKeyOptions),
    /// Creates a new wallet with optional <seed> and optional <password>, and prints the ID. Note the legacy --key option can still be used and will function the same as --seed. Use --wallet-list to retrieve the wallet ID in the future
    WalletCreate(WalletCreateOptions),
    /// Destroys <wallet> and all keys it contains
    WalletDestroy(WalletDestroyOptions),
    /// Insert <key> in to <wallet>
    WalletAddAdhoc(WalletAddAdhocOptions),
    /// Changes seed for <wallet> to <key>
    WalletChangeSeed(WalletChangeSeedOptions),
    /// Prints default representative for <wallet>
    WalletRepresentativeGet(WalletRepresentativeGetOptions),
    /// Set <account> as default representative for <wallet>
    WalletRepresentativeSet(WalletRepresentativeSetOptions),
    /// Remove <account> from <wallet>
    WalletRemove(WalletRemoveOptions),
    /// Decrypts <wallet> using <password>, !!THIS WILL PRINT YOUR PRIVATE KEY TO STDOUT!
    WalletDecryptUnsafe(WalletDecryptUnsafeOptions),
    /// Dumps wallet IDs and public keys
    WalletList(WalletListOptions),
    /// Clear online weight history records
    OnlineWeightClear(OnlineWeightClearOptions),
    /// Remove all send IDs from the database (dangerous: not intended for production use)
    ClearSendIds(ClearSendIdsOptions),
    /// Clear online peers database dump
    PeerClear(PeerClearOptions),
    /// Clear confirmation height. Requires an <account> option that can be 'all' to clear all accounts
    ConfirmationHeightClear(ConfirmationHeightClearOptions),
    /// Run internal diagnostics
    Diagnostics(DiagnosticsOptions),
}

pub(crate) fn get_path(path_str: &Option<String>, network_str: &Option<String>) -> PathBuf {
    if let Some(path) = path_str {
        return PathBuf::from_str(path).unwrap();
    }
    if let Some(network) = network_str {
        let network = Networks::from_str(&network).unwrap();
        NetworkConstants::set_active_network(network);
    }
    working_path().unwrap()
}
