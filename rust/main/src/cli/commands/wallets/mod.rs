use add_private_key::AddPrivateKeyArgs;
use anyhow::Result;
use change_wallet_seed::ChangeWalletSeedArgs;
use clap::{CommandFactory, Parser, Subcommand};
use create_account::CreateAccountArgs;
use create_wallet::CreateWalletArgs;
use decrypt_wallet::DecryptWalletArgs;
use destroy_wallet::DestroyWalletArgs;
use get_wallet_representative::GetWalletRepresentativeArgs;
use import_keys::ImportKeysArgs;
use list_wallets::ListWalletsArgs;
use remove_account::RemoveAccountArgs;
use send_ids::SendIdsArgs;
use set_wallet_representative::SetWalletRepresentativeArgs;

pub(crate) mod add_private_key;
pub(crate) mod change_wallet_seed;
pub(crate) mod create_account;
pub(crate) mod create_wallet;
pub(crate) mod decrypt_wallet;
pub(crate) mod destroy_wallet;
pub(crate) mod get_wallet_representative;
pub(crate) mod import_keys;
pub(crate) mod list_wallets;
pub(crate) mod remove_account;
pub(crate) mod send_ids;
pub(crate) mod set_wallet_representative;

#[derive(Subcommand)]
pub(crate) enum WalletSubcommands {
    /// Inserts next deterministic key in to <wallet>
    CreateAccount(CreateAccountArgs),
    /// Creates a new wallet with optional <seed> and optional <password>, and prints the ID.
    ///
    /// Note the legacy --key option can still be used and will function the same as --seed.
    /// Use --wallet-list to retrieve the wallet ID in the future.
    CreateWallet(CreateWalletArgs),
    /// Destroys <wallet> and all keys it contains.
    DestroyWallet(DestroyWalletArgs),
    /// Imports keys in <file> in to <wallet>.
    ImportKeys(ImportKeysArgs),
    /// Inserts <private_key> in to <wallet>.
    AddPrivateKey(AddPrivateKeyArgs),
    /// Changes seed for <wallet> to <key>.
    ChangeWalletSeed(ChangeWalletSeedArgs),
    /// Prints default representative for <wallet>.
    GetWalletRepresentative(GetWalletRepresentativeArgs),
    /// Sets account as default representative for <wallet>.
    SetWalletRepresentative(SetWalletRepresentativeArgs),
    /// Removes <account> from <wallet>.
    RemoveAccount(RemoveAccountArgs),
    /// Decrypts <wallet> using <password>, THIS WILL PRINT YOUR PRIVATE KEY TO STDOUT!
    DecryptWallet(DecryptWalletArgs),
    /// Dumps wallet IDs and public keys.
    ListWallets(ListWalletsArgs),
    /// Removes all send IDs from the database (dangerous: not intended for production use).
    SendIds(SendIdsArgs),
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
            Some(WalletSubcommands::ListWallets(args)) => args.list_wallets()?,
            Some(WalletSubcommands::CreateWallet(args)) => args.create_wallet()?,
            Some(WalletSubcommands::DestroyWallet(args)) => args.destroy_wallet()?,
            Some(WalletSubcommands::AddPrivateKey(args)) => args.add_key()?,
            Some(WalletSubcommands::ChangeWalletSeed(args)) => args.change_wallet_seed()?,
            Some(WalletSubcommands::ImportKeys(args)) => args.import_keys()?,
            Some(WalletSubcommands::RemoveAccount(args)) => args.remove_account()?,
            Some(WalletSubcommands::DecryptWallet(args)) => args.decrypt_wallet()?,
            Some(WalletSubcommands::GetWalletRepresentative(args)) => {
                args.get_wallet_representative()?
            }
            Some(WalletSubcommands::SetWalletRepresentative(args)) => {
                args.set_representative_wallet()?
            }
            Some(WalletSubcommands::SendIds(args)) => args.send_ids()?,
            None => WalletsCommand::command().print_long_help()?,
        }

        Ok(())
    }
}
