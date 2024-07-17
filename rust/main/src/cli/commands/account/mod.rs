use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use create::CreateArgs;
use get::GetArgs;
use key::KeyArgs;

pub(crate) mod create;
pub(crate) mod get;
pub(crate) mod key;

#[derive(Subcommand)]
pub(crate) enum AccountSubcommands {
    /// Insert next deterministic key into <wallet>.
    Create(CreateArgs),
    /// Get account number for the <key>.
    Get(GetArgs),
    /// Get the public key for <account>.
    Key(KeyArgs),
}

#[derive(Parser)]
pub(crate) struct AccountCommand {
    #[command(subcommand)]
    pub subcommand: Option<AccountSubcommands>,
}

impl AccountCommand {
    pub(crate) fn run(&self) -> Result<()> {
        match &self.subcommand {
            Some(AccountSubcommands::Create(args)) => args.create()?,
            Some(AccountSubcommands::Get(args)) => args.get()?,
            Some(AccountSubcommands::Key(args)) => args.key()?,
            None => AccountCommand::command().print_long_help()?,
        }

        Ok(())
    }
}
