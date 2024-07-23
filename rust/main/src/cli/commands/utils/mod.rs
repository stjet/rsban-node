use account_to_key::AccountToKeyArgs;
use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use key_expand::ExpandKeyArgs;
use key_to_account::KeyToAccountArgs;
use rsnano_core::{Account, KeyPair};

pub(crate) mod account_to_key;
pub(crate) mod key_expand;
pub(crate) mod key_to_account;

#[derive(Subcommand)]
pub(crate) enum UtilsSubcommands {
    /// Get account number for the <key>
    AccountGet(KeyToAccountArgs),
    /// Get the public key for <account>
    KeyGet(AccountToKeyArgs),
    /// Derive public key and account number from <key>
    KeyExpand(ExpandKeyArgs),
    /// Generates a adhoc random keypair and prints it to stdout
    KeyCreate,
}

#[derive(Parser)]
pub(crate) struct UtilsCommand {
    #[command(subcommand)]
    pub subcommand: Option<UtilsSubcommands>,
}

impl UtilsCommand {
    pub(crate) fn run(&self) -> Result<()> {
        match &self.subcommand {
            Some(UtilsSubcommands::AccountGet(args)) => args.key_to_account()?,
            Some(UtilsSubcommands::KeyGet(args)) => args.account_to_key()?,
            Some(UtilsSubcommands::KeyExpand(args)) => args.expand_key()?,
            Some(UtilsSubcommands::KeyCreate) => UtilsCommand::create_key(),
            None => UtilsCommand::command().print_long_help()?,
        }

        Ok(())
    }

    fn create_key() {
        let keypair = KeyPair::new();
        let private_key = keypair.private_key();
        let public_key = keypair.public_key();
        let account = Account::encode_account(&public_key);

        println!("Private: {:?}", private_key);
        println!("Public: {:?}", public_key);
        println!("Account: {:?}", account);
    }
}
