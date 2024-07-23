use account_count::AccountCountArgs;
use anyhow::Result;
use block_count::BlockCountArgs;
use blocks::Blocks;
use cemented_block_count::CementedBlockCountArgs;
use clap::{CommandFactory, Parser, Subcommand};
use compare_rep_weights::CompareRepWeightsArgs;
use peers::PeersArgs;
use representatives::RepresentativesArgs;
use trended_weight::TrendedWeightArgs;
use validate_blocks::ValidateBlocksArgs;

pub(crate) mod account_count;
pub(crate) mod block_count;
pub(crate) mod blocks;
pub(crate) mod cemented_block_count;
pub(crate) mod compare_rep_weights;
pub(crate) mod peers;
pub(crate) mod representatives;
pub(crate) mod trended_weight;
pub(crate) mod validate_blocks;

#[derive(Subcommand)]
pub(crate) enum ReadSubcommands {
    /// Displays the number of accounts
    AccountCount(AccountCountArgs),
    /// Displays the number of blocks
    BlockCount(BlockCountArgs),
    /// Displays all the blocks in the ledger in text format
    Blocks(Blocks),
    /// Displays peer IPv6:port connections
    Peers(PeersArgs),
    /// Displays the number of cemented (confirmed) blocks
    CementedBlockCount(CementedBlockCountArgs),
    //DumpFrontierUncheckedDependents(DumpFrontierUncheckedDependentsArgs),
    /// Lists representatives and weights
    Representatives(RepresentativesArgs),
    /// Dumps trended weights table
    TrendedWeight(TrendedWeightArgs),
    /// Displays a summarized comparison between the hardcoded bootstrap weights and representative weights from the ledger
    ///
    /// Full comparison is output to logs
    CompareRepWeights(CompareRepWeightsArgs),
    /// Checks all blocks for correct hash, signature, work value
    ValidateBlocks(ValidateBlocksArgs), // is this needed?
}

#[derive(Parser)]
pub(crate) struct ReadCommand {
    #[command(subcommand)]
    pub subcommand: Option<ReadSubcommands>,
}

impl ReadCommand {
    pub(crate) fn run(&self) -> Result<()> {
        match &self.subcommand {
            Some(ReadSubcommands::AccountCount(args)) => args.account_count()?,
            Some(ReadSubcommands::BlockCount(args)) => args.block_count()?,
            Some(ReadSubcommands::Blocks(args)) => args.block_dump()?,
            Some(ReadSubcommands::CementedBlockCount(args)) => args.cemented_block_count()?,
            Some(ReadSubcommands::Peers(args)) => args.peers()?,
            Some(ReadSubcommands::TrendedWeight(args)) => args.dump_trended_weight()?,
            Some(ReadSubcommands::Representatives(args)) => args.dump_representatives()?,
            Some(ReadSubcommands::CompareRepWeights(args)) => args.compare_rep_weights()?,
            Some(ReadSubcommands::ValidateBlocks(args)) => args.validate_blocks()?,
            None => ReadCommand::command().print_long_help()?,
        }

        Ok(())
    }
}
