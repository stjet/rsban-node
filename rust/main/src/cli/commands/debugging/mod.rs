use account_count::AccountCountArgs;
use anyhow::Result;
use block_count::BlockCountArgs;
use cemented_block_count::CementedBlockCountArgs;
use clap::{CommandFactory, Parser, Subcommand};
use peers::PeersArgs;

pub(crate) mod account_count;
pub(crate) mod block_count;
pub(crate) mod cemented_block_count;
pub(crate) mod peers;

#[derive(Subcommand)]
pub(crate) enum DebugSubcommands {
    /// Display the number of accounts
    AccountCount(AccountCountArgs),
    /// Display the total counts of each version for all accounts (including unpocketed)
    AccountVersions,
    /// Display the number of blocks
    BlockCount(BlockCountArgs),
    /// Display all the blocks in the ledger in text format
    BlockDump,
    /// Generate bootstrap sequence of blocks
    BootstrapGenerate,
    /// Displays the number of cemented (confirmed) blocks
    CementedBlockCount(CementedBlockCountArgs),
    /// Display a summarized comparison between the hardcoded bootstrap weights and representative weights from the ledger
    ///
    /// Full comparison is output to logs
    CompareRepWeights,
    /// Dump frontiers which have matching unchecked keys
    DumpFrontierUncheckedDependents,
    /// List representatives and weights
    DumpRepresentatives,
    /// Dump trended weights table
    DumpTrendedWeight,
    /// Consolidates the nano_node_backtrace.dump file
    ///
    /// Requires addr2line installed on Linux
    GenerateCrashReport,
    /// OpenCL work generation
    OpenCL,
    /// Displays the contents of the latest backtrace in the event of a nano_node crash
    OutputLastBacktraceDump,
    /// Display peer IPv6:port connections
    Peers(PeersArgs),
    /// Profile bootstrap style blocks processing (at least 10GB of free storage space required)
    ProfileBootstrap,
    /// Profile frontiers confirmation speed (only for nano_dev_network)
    ProfileFrontiersConfirmation,
    /// Profile work generation
    ProfileGenerate,
    /// Profile kdf function
    ProfileKdf,
    /// Profile active blocks processing (only for nano_dev_network)
    ProfileProcess,
    /// Profile signature generation
    ProfileSign,
    /// Profile work validation
    ProfileValidate,
    /// Profile votes processing (only for nano_dev_network)
    ProfileVotes,
    /// Prune accounts up to last confirmed blocks (EXPERIMENTAL)
    Prune,
    /// Generates output to RNG test suites
    RandomFeed,
    /// Read an RPC command from stdin and invoke it
    ///
    /// Network operations will have no effect
    Rpc,
    /// Display an example stacktrace
    Stacktrace,
    /// Test the system logger
    SysLogging,
    /// Displays the account, height (sorted), frontier and cemented frontier for all accounts which are not fully confirmed
    UnconfirmedFrontiers,
    /// Check all blocks for correct hash, signature, work value
    ValidateBlocks,
    /// Profile signature verification
    VerifyProfile,
    /// Profile batch signature verification
    VerifyProfileBatch,
}

#[derive(Parser)]
pub(crate) struct DebugCommand {
    #[command(subcommand)]
    pub subcommand: Option<DebugSubcommands>,
}

impl DebugCommand {
    pub(crate) fn run(&self) -> Result<()> {
        match &self.subcommand {
            Some(DebugSubcommands::AccountCount(args)) => args.account_count()?,
            Some(DebugSubcommands::AccountVersions) => DebugCommand::account_version(),
            Some(DebugSubcommands::BlockCount(args)) => args.block_count()?,
            Some(DebugSubcommands::BlockDump) => DebugCommand::block_dump(),
            Some(DebugSubcommands::BootstrapGenerate) => DebugCommand::bootstrap_generate(),
            Some(DebugSubcommands::CementedBlockCount(args)) => args.cemented_block_count()?,
            Some(DebugSubcommands::CompareRepWeights) => DebugCommand::compare_rep_weights(),
            Some(DebugSubcommands::DumpFrontierUncheckedDependents) => {
                DebugCommand::dump_frontier_unchecked_dependents()
            }
            Some(DebugSubcommands::DumpRepresentatives) => DebugCommand::dump_representatives(),
            Some(DebugSubcommands::DumpTrendedWeight) => DebugCommand::dump_trended_weight(),
            Some(DebugSubcommands::GenerateCrashReport) => DebugCommand::generate_crash_report(),
            Some(DebugSubcommands::OpenCL) => DebugCommand::opencl(),
            Some(DebugSubcommands::OutputLastBacktraceDump) => {
                DebugCommand::output_last_backtrace_dump()
            }
            Some(DebugSubcommands::Peers(args)) => args.peers()?,
            Some(DebugSubcommands::ProfileBootstrap) => DebugCommand::profile_bootstrap(),
            Some(DebugSubcommands::ProfileFrontiersConfirmation) => {
                DebugCommand::profile_frontier_confirmation()
            }
            Some(DebugSubcommands::ProfileGenerate) => DebugCommand::profile_generate(),
            Some(DebugSubcommands::ProfileKdf) => DebugCommand::profile_kdf(),
            Some(DebugSubcommands::ProfileProcess) => DebugCommand::profile_process(),
            Some(DebugSubcommands::ProfileSign) => DebugCommand::profile_sign(),
            Some(DebugSubcommands::ProfileValidate) => DebugCommand::profile_validate(),
            Some(DebugSubcommands::ProfileVotes) => DebugCommand::profile_votes(),
            Some(DebugSubcommands::Prune) => DebugCommand::prune(),
            Some(DebugSubcommands::RandomFeed) => DebugCommand::random_feed(),
            Some(DebugSubcommands::Rpc) => DebugCommand::rpc(),
            Some(DebugSubcommands::Stacktrace) => DebugCommand::stacktrace(),
            Some(DebugSubcommands::SysLogging) => DebugCommand::sys_logging(),
            Some(DebugSubcommands::UnconfirmedFrontiers) => DebugCommand::unconfirmed_frontiers(),
            Some(DebugSubcommands::ValidateBlocks) => DebugCommand::validate_blocks(),
            Some(DebugSubcommands::VerifyProfile) => DebugCommand::verify_profile(),
            Some(DebugSubcommands::VerifyProfileBatch) => DebugCommand::verify_profile_batch(),
            None => DebugCommand::command().print_long_help()?,
        }

        Ok(())
    }

    fn account_version() {
        println!("Running account_version");
        // Implement the logic for account_version
    }

    fn block_dump() {
        // Implement the logic for block_dump
    }

    fn bootstrap_generate() {
        // Implement the logic for bootstrap_generate
    }

    fn compare_rep_weights() {
        // Implement the logic for compare_rep_weights
    }

    fn dump_frontier_unchecked_dependents() {
        println!("Running dump_frontier_unchecked_dependents");
        // Implement the logic for dump_frontier_unchecked_dependents
    }

    fn dump_representatives() {
        println!("Running dump_representatives");
        // Implement the logic for dump_representatives
    }

    fn dump_trended_weight() {
        println!("Running dump_trended_weight");
        // Implement the logic for dump_trended_weight
    }

    fn generate_crash_report() {
        println!("Running generate_crash_report");
        // Implement the logic for generate_crash_report
    }

    fn opencl() {
        // Implement the logic for opencl
    }

    fn output_last_backtrace_dump() {
        println!("Running output_last_backtrace_dump");
        // Implement the logic for output_last_backtrace_dump
    }

    fn profile_bootstrap() {
        // Implement the logic for profile_bootstrap
    }

    fn profile_frontier_confirmation() {
        println!("Running profile_frontier_confirmation");
        // Implement the logic for profile_frontier_confirmation
    }

    fn profile_generate() {
        println!("Running profile_generate");
        // Implement the logic for profile_generate
    }

    fn profile_kdf() {
        println!("Running profile_kdf");
        // Implement the logic for profile_kdf
    }

    fn profile_process() {
        println!("Running profile_process");
        // Implement the logic for profile_process
    }

    fn profile_sign() {
        println!("Running profile_sign");
        // Implement the logic for profile_sign
    }

    fn profile_validate() {
        println!("Running profile_validate");
        // Implement the logic for profile_validate
    }

    fn profile_votes() {
        println!("Running profile_votes");
        // Implement the logic for profile_votes
    }

    fn prune() {
        println!("Running prune");
        // Implement the logic for prune
    }

    fn random_feed() {
        println!("Running random_feed");
        // Implement the logic for random_feed
    }

    fn rpc() {
        println!("Running rpc");
        // Implement the logic for rpc
    }

    fn stacktrace() {
        println!("Running stacktrace");
        // Implement the logic for stacktrace
    }

    fn sys_logging() {
        println!("Running sys_logging");
        // Implement the logic for sys_logging
    }

    fn unconfirmed_frontiers() {
        // Implement the logic for unconfirmed_frontiers
    }

    fn validate_blocks() {
        println!("Running validate_blocks");
        // Implement the logic for validate_blocks
    }

    fn verify_profile() {
        println!("Running verify_profile");
        // Implement the logic for verify_profile
    }

    fn verify_profile_batch() {
        // Implement the logic for verify_profile_batch
    }
}
