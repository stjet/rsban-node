use clap::{ArgGroup, Parser};

#[derive(Parser)]
#[command(group = ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct WalletImportOptions {
    #[arg(short, long)]
    file: String,
    #[arg(short, long)]
    password: Option<String>,
    #[arg(long)]
    force: Option<bool>,
    #[arg(long)]
    wallet: String,
    #[arg(long, group = "input")]
    data_path: Option<String>,
    #[arg(long, group = "input")]
    network: Option<String>,
}
