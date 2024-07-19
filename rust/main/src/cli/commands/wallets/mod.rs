use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use create_account::CreateAccountArgs;
use {
    add_adhoc::AddAdhocArgs, change_seed::ChangeSeedArgs, create::CreateWalletArgs,
    decrypt_unsafe::DecryptUnsafeArgs, destroy::DestroyWalletArgs, import::ImportKeysArgs,
    list::ListArgs, remove::RemoveArgs, representative_get::RepresentativeGetArgs,
    representative_set::RepresentativeSetArgs,
};

pub(crate) mod add_adhoc;
pub(crate) mod change_seed;
pub(crate) mod create;
pub(crate) mod create_account;
pub(crate) mod decrypt_unsafe;
pub(crate) mod destroy;
pub(crate) mod import;
pub(crate) mod list;
pub(crate) mod remove;
pub(crate) mod representative_get;
pub(crate) mod representative_set;

#[derive(Subcommand)]
pub(crate) enum WalletSubcommands {
    /// Insert next deterministic key in to <wallet>
    CreateAccount(CreateAccountArgs),
    /// Creates a new wallet with optional <seed> and optional <password>, and prints the ID.
    ///
    /// Note the legacy --key option can still be used and will function the same as --seed.
    /// Use --wallet-list to retrieve the wallet ID in the future.
    CreateWallet(CreateWalletArgs),
    /// Destroys <wallet> and all keys it contains.
    DestroyWallet(DestroyWalletArgs),
    /// Imports keys in <file> using <password> in to <wallet>.
    ImportKeys(ImportKeysArgs),
    /// Insert <key> in to <wallet>.
    AddAdhoc(AddAdhocArgs),
    /// Changes seed for <wallet> to <key>.
    ChangeSeed(ChangeSeedArgs),
    /// Prints default representative for <wallet>.
    RepresentativeGet(RepresentativeGetArgs),
    /// Set <account> as default representative for <wallet>.
    RepresentativeSet(RepresentativeSetArgs),
    /// Remove <account> from <wallet>.
    Remove(RemoveArgs),
    /// Decrypts <wallet> using <password>, !!THIS WILL PRINT YOUR PRIVATE KEY TO STDOUT!
    DecryptUnsafe(DecryptUnsafeArgs),
    /// Dumps wallet IDs and public keys.
    List(ListArgs),
}

#[derive(Parser)]
pub(crate) struct WalletsCommand {
    #[command(subcommand)]
    pub subcommand: Option<WalletSubcommands>,
}

impl WalletsCommand {
    pub(crate) fn run(&self) -> Result<()> {
        match &self.subcommand {
            Some(WalletSubcommands::CreateAccount(args)) => args.create_account()?,
            Some(WalletSubcommands::List(args)) => args.list()?,
            Some(WalletSubcommands::CreateWallet(args)) => args.create_wallet()?,
            Some(WalletSubcommands::DestroyWallet(args)) => args.destroy_wallet()?,
            Some(WalletSubcommands::AddAdhoc(args)) => args.add_adhoc()?,
            Some(WalletSubcommands::ChangeSeed(args)) => args.change_seed()?,
            Some(WalletSubcommands::ImportKeys(args)) => args.import()?,
            Some(WalletSubcommands::Remove(args)) => args.remove()?,
            Some(WalletSubcommands::DecryptUnsafe(args)) => args.decrypt_unsafe()?,
            Some(WalletSubcommands::RepresentativeGet(args)) => args.representative_get()?,
            Some(WalletSubcommands::RepresentativeSet(args)) => args.representative_set()?,
            None => WalletsCommand::command().print_long_help()?,
        }

        Ok(())
    }
}
