use account_count::AccountCountArgs;
use anyhow::Result;
use block_count::BlockCountArgs;
use block_dump::Blocks;
use cemented_block_count::CementedBlockCountArgs;
use clap::{CommandFactory, Parser, Subcommand};
use dump_representatives::RepresentativesArgs;
use dump_trended_weight::TrendedWeightArgs;
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
    /// Display the number of accounts
    AccountCount(AccountCountArgs),
    /// Display the number of blocks
    BlockCount(BlockCountArgs),
    /// RDisplay all the blocks in the ledger in text format
    Blocks(Blocks),
    /// Display peer IPv6:port connections
    Peers(PeersArgs),
    /// Displays the number of cemented (confirmed) blocks
    CementedBlockCount(CementedBlockCountArgs),
    //DumpFrontierUncheckedDependents(DumpFrontierUncheckedDependentsArgs),
    /// List representatives and weights
    Representatives(RepresentativesArgs),
    /// Dump trended weights table
    TrendedWeight(TrendedWeightArgs),
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
            Some(InfoSubcommands::Blocks(args)) => args.block_dump()?,
            Some(InfoSubcommands::CementedBlockCount(args)) => args.cemented_block_count()?,
            Some(InfoSubcommands::Peers(args)) => args.peers()?,
            Some(InfoSubcommands::TrendedWeight(args)) => args.dump_trended_weight()?,
            Some(InfoSubcommands::Representatives(args)) => args.dump_representatives()?,
            None => InfoCommand::command().print_long_help()?,
        }

        Ok(())
    }
}
