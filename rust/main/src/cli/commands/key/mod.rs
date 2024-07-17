use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use expand::ExpandArgs;
use rsnano_core::{Account, KeyPair};

pub(crate) mod expand;

#[derive(Subcommand)]
pub(crate) enum KeySubcommands {
    /// Generates a adhoc random keypair and prints it to stdout.
    Create,
    /// Derive public key and account number from <key>.
    Expand(ExpandArgs),
}

#[derive(Parser)]
pub(crate) struct KeyCommand {
    #[command(subcommand)]
    pub subcommand: Option<KeySubcommands>,
}

impl KeyCommand {
    pub(crate) fn run(&self) -> Result<()> {
        match &self.subcommand {
            Some(KeySubcommands::Create) => KeyCommand::create(),
            Some(KeySubcommands::Expand(args)) => args.expand()?,
            None => KeyCommand::command().print_long_help()?,
        }

        Ok(())
    }

    fn create() {
        let keypair = KeyPair::new();
        let private_key = keypair.private_key();
        let public_key = keypair.public_key();
        let account = Account::encode_account(&public_key);

        println!("Private: {:?}", private_key);
        println!("Public: {:?}", public_key);
        println!("Account: {:?}", account);
    }
}
