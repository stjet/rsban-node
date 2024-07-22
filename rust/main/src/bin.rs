use clap::Parser;
use cli::Cli;

mod cli;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    cli.run()?;
    Ok(())
}
