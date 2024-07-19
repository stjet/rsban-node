use account_count::AccountCountArgs;
use anyhow::Result;
use block_count::BlockCountArgs;
use block_dump::BlockDumpArgs;
use cemented_block_count::CementedBlockCountArgs;
use clap::{CommandFactory, Parser, Subcommand};
use dump_representatives::DumpRepresentativesArgs;
use dump_trended_weight::DumpTrendedWeightArgs;
use peers::PeersArgs;

pub(crate) mod account_count;
pub(crate) mod block_count;
pub(crate) mod block_dump;
pub(crate) mod cemented_block_count;
pub(crate) mod dump_representatives;
pub(crate) mod dump_trended_weight;
pub(crate) mod peers;

#[derive(Subcommand)]
pub(crate) enum InfoSubcommands {
    /// Either specify a single --root to clear or --all to clear all final votes (not recommended).
    AccountCount(AccountCountArgs),
    /// Clear online weight history records.
    BlockCount(BlockCountArgs),
    /// Remove all send IDs from the database (dangerous: not intended for production use).
    BlockDump(BlockDumpArgs),
    /// Clear online peers database dump.
    Peers(PeersArgs),
    /// Clear confirmation height. Requires an <account> option that can be 'all' to clear all accounts.
    CementedBlockCount(CementedBlockCountArgs),
    //DumpFrontierUncheckedDependents(DumpFrontierUncheckedDependentsArgs),
    DumpRepresentatives(DumpRepresentativesArgs),
    DumpTrendedWeight(DumpTrendedWeightArgs),
}

#[derive(Parser)]
pub(crate) struct InfoCommand {
    #[command(subcommand)]
    pub subcommand: Option<InfoSubcommands>,
}

impl InfoCommand {
    pub(crate) fn run(&self) -> Result<()> {
        match &self.subcommand {
            Some(InfoSubcommands::AccountCount(args)) => args.account_count()?,
            Some(InfoSubcommands::BlockCount(args)) => args.block_count()?,
            Some(InfoSubcommands::BlockDump(args)) => args.block_dump()?,
            Some(InfoSubcommands::CementedBlockCount(args)) => args.cemented_block_count()?,
            Some(InfoSubcommands::Peers(args)) => args.peers()?,
            Some(InfoSubcommands::DumpTrendedWeight(args)) => args.dump_trended_weight()?,
            Some(InfoSubcommands::DumpRepresentatives(args)) => args.dump_representatives()?,
            None => InfoCommand::command().print_long_help()?,
        }

        Ok(())
    }
}
