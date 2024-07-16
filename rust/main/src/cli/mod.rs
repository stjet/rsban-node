use clap::{CommandFactory, Parser, Subcommand};
use commands::{
    accounts::{
        account_create::AccountCreateArgs, account_get::AccountGetArgs, account_key::AccountKeyArgs,
    },
    clear::{
        clear_send_ids::ClearSendIdsArgs, confirmation_height_clear::ConfirmationHeightClearArgs,
        final_vote_clear::FinalVoteClearArgs, online_weight_clear::OnlineWeightClearArgs,
        peer_clear::PeerClearArgs,
    },
    database::{rebuild_database::RebuildDatabaseArgs, snapshot::SnapshotArgs, vacuum::VacuumArgs},
    keys::key_expand::KeyExpandArgs,
    node::{daemon::DaemonArgs, generate_config::GenerateConfigArgs, initialize::InitializeArgs},
    wallets::{
        wallet_add_adhoc::WalletAddAdhocArgs, wallet_change_seed::WalletChangeSeedArgs,
        wallet_create::WalletCreateArgs, wallet_decrypt_unsafe::WalletDecryptUnsafeArgs,
        wallet_destroy::WalletDestroyArgs, wallet_import::WalletImportArgs,
        wallet_list::WalletListArgs, wallet_remove::WalletRemoveArgs,
        wallet_representative_get::WalletRepresentativeGetArgs,
        wallet_representative_set::WalletRepresentativeSetArgs,
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
            Some(Commands::Daemon(args)) => {
                args.daemon();
            }
            Some(Commands::Initialize(args)) => {
                args.initialize();
            }
            Some(Commands::OnlineWeightClear(args)) => {
                args.online_weight_clear();
            }
            Some(Commands::PeerClear(args)) => {
                args.peer_clear();
            }
            Some(Commands::ConfirmationHeightClear(args)) => {
                args.confirmation_height_clear();
            }
            Some(Commands::ClearSendIds(args)) => {
                args.clear_send_ids();
            }
            Some(Commands::FinalVoteClear(args)) => args.final_vote_clear()?,
            Some(Commands::KeyCreate) => {
                Cli::key_create();
            }
            Some(Commands::WalletList(args)) => {
                args.wallet_list();
            }
            Some(Commands::WalletCreate(args)) => args.wallet_create()?,
            Some(Commands::WalletDestroy(args)) => {
                args.wallet_destroy();
            }
            Some(Commands::WalletAddAdhoc(args)) => {
                args.wallet_add_adhoc();
            }
            Some(Commands::WalletChangeSeed(args)) => {
                args.wallet_change_seed();
            }
            Some(Commands::WalletImport(args)) => args.wallet_import()?,
            Some(Commands::WalletRemove(args)) => {
                args.wallet_remove();
            }
            Some(Commands::WalletDecryptUnsafe(args)) => {
                args.wallet_decrypt_unsafe();
            }
            Some(Commands::WalletRepresentativeGet(args)) => {
                args.wallet_representative_get();
            }
            Some(Commands::WalletRepresentativeSet(args)) => {
                args.wallet_representative_set();
            }
            Some(Commands::AccountGet(args)) => {
                args.account_get();
            }
            Some(Commands::AccountKey(args)) => {
                args.account_key();
            }
            Some(Commands::AccountCreate(args)) => {
                args.account_create();
            }
            Some(Commands::KeyExpand(args)) => {
                args.key_expand();
            }
            Some(Commands::Diagnostics) => {
                Cli::diagnostics();
            }
            Some(Commands::Version) => {
                Cli::version();
            }
            Some(Commands::Vacuum(args)) => args.vacuum()?,
            Some(Commands::RebuildDatabase(args)) => {
                args.rebuild_database();
            }
            Some(Commands::Snapshot(args)) => args.snapshot()?,
            Some(Commands::GenerateConfig(args)) => args.generate_config()?,
            None => Cli::command().print_help()?,
        }
        Ok(())
    }

    fn version() {
        println!("Version {}", VERSION_STRING);
        println!("Build Info {}", BUILD_INFO);
    }

    fn diagnostics() {
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

    fn key_create() {
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
    Daemon(DaemonArgs),
    /// Initialize the data folder, if it is not already initialised. This command is meant to be run when the data folder is empty, to populate it with the genesis block.
    Initialize(InitializeArgs),
    /// Generates a adhoc random keypair and prints it to stdout.
    KeyCreate,
    /// Derive public key and account number from <key>.
    KeyExpand(KeyExpandArgs),
    /// Either specify a single --root to clear or --all to clear all final votes (not recommended).
    FinalVoteClear(FinalVoteClearArgs),
    /// Insert next deterministic key into <wallet>.
    AccountCreate(AccountCreateArgs),
    /// Get account number for the <key>.
    AccountGet(AccountGetArgs),
    /// Get the public key for <account>.
    AccountKey(AccountKeyArgs),
    /// Creates a new wallet with optional <seed> and optional <password>, and prints the ID. Note the legacy --key option can still be used and will function the same as --seed. Use --wallet-list to retrieve the wallet ID in the future.
    WalletCreate(WalletCreateArgs),
    /// Destroys <wallet> and all keys it contains.
    WalletDestroy(WalletDestroyArgs),
    /// Imports keys in <file> using <password> in to <wallet>.
    WalletImport(WalletImportArgs),
    /// Insert <key> in to <wallet>.
    WalletAddAdhoc(WalletAddAdhocArgs),
    /// Changes seed for <wallet> to <key>.
    WalletChangeSeed(WalletChangeSeedArgs),
    /// Prints default representative for <wallet>.
    WalletRepresentativeGet(WalletRepresentativeGetArgs),
    /// Set <account> as default representative for <wallet>.
    WalletRepresentativeSet(WalletRepresentativeSetArgs),
    /// Remove <account> from <wallet>.
    WalletRemove(WalletRemoveArgs),
    /// Decrypts <wallet> using <password>, !!THIS WILL PRINT YOUR PRIVATE KEY TO STDOUT!
    WalletDecryptUnsafe(WalletDecryptUnsafeArgs),
    /// Dumps wallet IDs and public keys.
    WalletList(WalletListArgs),
    /// Clear online weight history records.
    OnlineWeightClear(OnlineWeightClearArgs),
    /// Remove all send IDs from the database (dangerous: not intended for production use).
    ClearSendIds(ClearSendIdsArgs),
    /// Clear online peers database dump.
    PeerClear(PeerClearArgs),
    /// Clear confirmation height. Requires an <account> option that can be 'all' to clear all accounts.
    ConfirmationHeightClear(ConfirmationHeightClearArgs),
    /// Run internal diagnostics.
    Diagnostics,
    /// Compact database. If data_path is missing, the database in the data directory is compacted. Optional: --unchecked_clear, --clear_send_ids, --online_weight_clear, --peer_clear, --confirmation_height_clear, --rebuild_database. Requires approximately data.ldb size * 2 free space on disk.
    Vacuum(VacuumArgs),
    /// Rebuild LMDB database with --vacuum for best compaction. Requires approximately data.ldb size * 2 free space on disk.
    RebuildDatabase(RebuildDatabaseArgs),
    /// Compact database and create snapshot, functions similar to vacuum but does not replace the existing database.
    Snapshot(SnapshotArgs),
    /// Prints out version.
    Version,
    /// Write configuration to stdout, populated with defaults suitable for this system. Pass the configuration type node or rpc. See also use_defaults.
    GenerateConfig(GenerateConfigArgs),
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
