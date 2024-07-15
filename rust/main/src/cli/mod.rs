use clap::{CommandFactory, Parser, Subcommand};
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
    database::{
        rebuild_database::RebuildDatabaseOptions, snapshot::SnapshotOptions, vacuum::VacuumOptions,
    },
    keys::key_expand::KeyExpandOptions,
    node::{
        daemon::DaemonOptions, generate_config::GenerateConfigOptions,
        initialize::InitializeOptions,
    },
    wallets::{
        wallet_add_adhoc::WalletAddAdhocOptions, wallet_change_seed::WalletChangeSeedOptions,
        wallet_create::WalletCreateOptions, wallet_decrypt_unsafe::WalletDecryptUnsafeOptions,
        wallet_destroy::WalletDestroyOptions, wallet_list::WalletListOptions,
        wallet_remove::WalletRemoveOptions,
        wallet_representative_get::WalletRepresentativeGetOptions,
        wallet_representative_set::WalletRepresentativeSetOptions,
    },
};
use rsnano_core::{Account, Amount, BlockHash, KeyPair, Networks, PublicKey, RawKey, SendBlock};
use rsnano_node::{
    config::NetworkConstants, wallets::Wallets, working_path, BUILD_INFO, VERSION_STRING,
};
use std::{path::PathBuf, str::FromStr, sync::Arc, time::Instant};

mod commands;

#[derive(Parser)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

impl Cli {
    pub(crate) fn run(&self) -> anyhow::Result<()> {
        match &self.command {
            Some(Commands::Daemon(daemon_options)) => {
                daemon_options.run();
            }
            Some(Commands::Initialize(initialize_options)) => {
                initialize_options.run();
            }
            Some(Commands::OnlineWeightClear(online_weight_clear_options)) => {
                online_weight_clear_options.run();
            }
            Some(Commands::PeerClear(peer_clear_options)) => {
                peer_clear_options.run();
            }
            Some(Commands::ConfirmationHeightClear(confirmation_height_clear_options)) => {
                confirmation_height_clear_options.run();
            }
            Some(Commands::ClearSendIds(clear_send_ids_options)) => {
                clear_send_ids_options.run();
            }
            Some(Commands::FinalVoteClear(final_vote_clear_options)) => {
                final_vote_clear_options.final_vote_clear()?;
            }
            Some(Commands::KeyCreate) => {
                self.key_create();
            }
            Some(Commands::WalletList(wallet_list_options)) => {
                wallet_list_options.run();
            }
            Some(Commands::WalletCreate(wallet_create_options)) => {
                wallet_create_options.run()?;
            }
            Some(Commands::WalletDestroy(wallet_destroy_options)) => {
                wallet_destroy_options.run();
            }
            Some(Commands::WalletAddAdhoc(wallet_destroy_options)) => {
                wallet_destroy_options.run();
            }
            Some(Commands::WalletChangeSeed(wallet_change_seed_options)) => {
                wallet_change_seed_options.run();
            }
            Some(Commands::WalletRemove(wallet_remove_options)) => {
                wallet_remove_options.run();
            }
            Some(Commands::WalletDecryptUnsafe(wallet_decrypt_unsafe_options)) => {
                wallet_decrypt_unsafe_options.run();
            }
            Some(Commands::WalletRepresentativeGet(wallet_representative_get_options)) => {
                wallet_representative_get_options.run();
            }
            Some(Commands::WalletRepresentativeSet(wallet_representative_set_options)) => {
                wallet_representative_set_options.run();
            }
            Some(Commands::AccountGet(account_get_options)) => {
                account_get_options.run();
            }
            Some(Commands::AccountKey(account_key_options)) => {
                account_key_options.run();
            }
            Some(Commands::AccountCreate(account_create_options)) => {
                account_create_options.run();
            }
            Some(Commands::KeyExpand(key_expand_options)) => {
                key_expand_options.run();
            }
            Some(Commands::Diagnostics) => {
                self.diagnostics();
            }
            Some(Commands::Version) => {
                self.version();
            }
            Some(Commands::Vacuum(vacuum_options)) => {
                vacuum_options.run();
            }
            Some(Commands::RebuildDatabase(rebuild_database_options)) => {
                rebuild_database_options.run();
            }
            Some(Commands::Snapshot(snapshot_options)) => {
                snapshot_options.run();
            }
            Some(Commands::Help) => {
                Cli::command().print_help()?;
            }
            Some(Commands::GenerateConfig(generate_config_options)) => {
                generate_config_options.run();
            }
            None => {
                Cli::command().print_help()?;
            }
        }
        Ok(())
    }

    fn version(&self) {
        println!("Version {}", VERSION_STRING);
        println!("Build Info {}", BUILD_INFO);
    }

    fn diagnostics(&self) {
        let path = get_path(&None, &None);

        let wallets = Arc::new(Wallets::new_null(&path).unwrap());

        println!("Testing hash function");

        SendBlock::new(
            &BlockHash::zero(),
            &Account::zero(),
            &Amount::zero(),
            &RawKey::zero(),
            &PublicKey::zero(),
            0,
        );

        println!("Testing key derivation function");

        wallets.kdf.hash_password("", &mut [0; 32]);

        println!("Testing time retrieval latency...");

        let iters = 2_000_000;
        let start = Instant::now();
        for _ in 0..iters {
            let _ = Instant::now();
        }
        let duration = start.elapsed();
        let avg_duration = duration.as_nanos() as f64 / iters as f64;

        println!("{} nanoseconds", avg_duration);
    }

    fn key_create(&self) {
        let keypair = KeyPair::new();
        let private_key = keypair.private_key();
        let public_key = keypair.public_key();
        let account = Account::encode_account(&public_key);

        println!("Private: {:?}", private_key);
        println!("Public: {:?}", public_key);
        println!("Account: {:?}", account);
    }
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Start node daemon.
    Daemon(DaemonOptions),
    /// Initialize the data folder, if it is not already initialised. This command is meant to be run when the data folder is empty, to populate it with the genesis block.
    Initialize(InitializeOptions),
    /// Generates a adhoc random keypair and prints it to stdout.
    KeyCreate,
    /// Derive public key and account number from <key>.
    KeyExpand(KeyExpandOptions),
    /// Either specify a single --root to clear or --all to clear all final votes (not recommended).
    FinalVoteClear(FinalVoteClearOptions),
    /// Insert next deterministic key into <wallet>.
    AccountCreate(AccountCreateOptions),
    /// Get account number for the <key>.
    AccountGet(AccountGetOptions),
    /// Get the public key for <account>.
    AccountKey(AccountKeyOptions),
    /// Creates a new wallet with optional <seed> and optional <password>, and prints the ID. Note the legacy --key option can still be used and will function the same as --seed. Use --wallet-list to retrieve the wallet ID in the future.
    WalletCreate(WalletCreateOptions),
    /// Destroys <wallet> and all keys it contains.
    WalletDestroy(WalletDestroyOptions),
    /// Insert <key> in to <wallet>.
    WalletAddAdhoc(WalletAddAdhocOptions),
    /// Changes seed for <wallet> to <key>.
    WalletChangeSeed(WalletChangeSeedOptions),
    /// Prints default representative for <wallet>.
    WalletRepresentativeGet(WalletRepresentativeGetOptions),
    /// Set <account> as default representative for <wallet>.
    WalletRepresentativeSet(WalletRepresentativeSetOptions),
    /// Remove <account> from <wallet>.
    WalletRemove(WalletRemoveOptions),
    /// Decrypts <wallet> using <password>, !!THIS WILL PRINT YOUR PRIVATE KEY TO STDOUT!
    WalletDecryptUnsafe(WalletDecryptUnsafeOptions),
    /// Dumps wallet IDs and public keys.
    WalletList(WalletListOptions),
    /// Clear online weight history records.
    OnlineWeightClear(OnlineWeightClearOptions),
    /// Remove all send IDs from the database (dangerous: not intended for production use).
    ClearSendIds(ClearSendIdsOptions),
    /// Clear online peers database dump.
    PeerClear(PeerClearOptions),
    /// Clear confirmation height. Requires an <account> option that can be 'all' to clear all accounts.
    ConfirmationHeightClear(ConfirmationHeightClearOptions),
    /// Run internal diagnostics.
    Diagnostics,
    /// Compact database. If data_path is missing, the database in the data directory is compacted. Optional: --unchecked_clear, --clear_send_ids, --online_weight_clear, --peer_clear, --confirmation_height_clear, --rebuild_database. Requires approximately data.ldb size * 2 free space on disk.
    Vacuum(VacuumOptions),
    /// Rebuild LMDB database with --vacuum for best compaction. Requires approximately data.ldb size * 2 free space on disk.
    RebuildDatabase(RebuildDatabaseOptions),
    /// Compact database and create snapshot, functions similar to vacuum but does not replace the existing database.
    Snapshot(SnapshotOptions),
    /// Prints out version.
    Version,
    /// Print out options.
    Help,
    /// Write configuration to stdout, populated with defaults suitable for this system. Pass the configuration type node or rpc. See also use_defaults.
    GenerateConfig(GenerateConfigOptions),
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
