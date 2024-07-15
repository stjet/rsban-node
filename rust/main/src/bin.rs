use clap::Parser;
use cli::Cli;
use tracing_subscriber::EnvFilter;

mod cli;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    cli.run()?;
    Ok(())
}

fn init_tracing(dirs: impl AsRef<str>) {
    let filter = EnvFilter::builder().parse_lossy(dirs);
    let value = std::env::var("NANO_LOG");
    let log_style = value.as_ref().map(|i| i.as_str()).unwrap_or_default();
    match log_style {
        "json" => {
            tracing_subscriber::fmt::fmt()
                .json()
                .with_env_filter(filter)
                .init();
        }
        "noansi" => {
            tracing_subscriber::fmt::fmt()
                .with_env_filter(filter)
                .with_ansi(false)
                .init();
        }
        _ => {
            tracing_subscriber::fmt::fmt()
                .with_env_filter(filter)
                .with_ansi(true)
                .init();
        }
    }
    tracing::debug!(log_style, ?value, "init tracing");
}
