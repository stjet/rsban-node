use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use read::ReadCommand;
use write::WriteCommand;

pub(crate) mod read;
pub(crate) mod write;

#[derive(Subcommand)]
pub(crate) enum LedgerSubcommands {
    /// Commands that read from the ledger
    Read(ReadCommand),
    /// Commands that write to the ledger
    Write(WriteCommand),
}

#[derive(Parser)]
pub(crate) struct LedgerCommand {
    #[command(subcommand)]
    pub subcommand: Option<LedgerSubcommands>,
}

impl LedgerCommand {
    pub(crate) fn run(&self) -> Result<()> {
        match &self.subcommand {
            Some(LedgerSubcommands::Read(command)) => command.run()?,
            Some(LedgerSubcommands::Write(command)) => command.run()?,
            None => LedgerCommand::command().print_long_help()?,
        }

        Ok(())
    }
}
