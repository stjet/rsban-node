use clap::{CommandFactory, Parser};
use cli::{Cli, Commands};
use tracing_subscriber::EnvFilter;

mod cli;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match &cli.command {
        Some(Commands::Daemon(daemon)) => {
            daemon.run();
        }
        Some(Commands::Initialize(initialize)) => {
            initialize.run();
        }
        Some(Commands::OnlineWeightClear(online_weight_clear)) => {
            online_weight_clear.run();
        }
        Some(Commands::PeerClear(peer_clear)) => {
            peer_clear.run();
        }
        Some(Commands::ConfirmationHeightClear(confirmation_height_clear)) => {
            confirmation_height_clear.run();
        }
        Some(Commands::ClearSendIds(clear_send_ids)) => {
            clear_send_ids.run();
        }
        Some(Commands::FinalVoteClear(final_vote_clear)) => {
            final_vote_clear.final_vote_clear()?;
        }
        Some(Commands::KeyCreate(key_create)) => {
            key_create.run();
        }
        Some(Commands::WalletList(wallet_list)) => {
            wallet_list.run();
        }
        Some(Commands::WalletCreate(wallet_create)) => {
            wallet_create.run()?;
        }
        Some(Commands::WalletDestroy(wallet_destroy)) => {
            wallet_destroy.run();
        }
        Some(Commands::WalletAddAdhoc(wallet_destroy)) => {
            wallet_destroy.run();
        }
        Some(Commands::WalletChangeSeed(wallet_change_seed)) => {
            wallet_change_seed.run();
        }
        Some(Commands::WalletRemove(wallet_remove)) => {
            wallet_remove.run();
        }
        Some(Commands::WalletDecryptUnsafe(wallet_decrypt_unsafe)) => {
            wallet_decrypt_unsafe.run();
        }
        Some(Commands::WalletRepresentativeGet(wallet_representative_get)) => {
            wallet_representative_get.run();
        }
        Some(Commands::WalletRepresentativeSet(wallet_representative_set)) => {
            wallet_representative_set.run();
        }
        Some(Commands::AccountGet(account_get)) => {
            account_get.run();
        }
        Some(Commands::AccountKey(account_key)) => {
            account_key.run();
        }
        Some(Commands::AccountCreate(account_create)) => {
            account_create.run();
        }
        Some(Commands::KeyExpand(key_expand)) => {
            key_expand.run();
        }
        Some(Commands::Diagnostics(diagnostics)) => {
            diagnostics.run();
        }
        None => {
            Cli::command().print_help()?;
        }
    }
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
