use crate::cli::get_path;
use clap::{ArgGroup, Parser};
use rsnano_store_lmdb::LmdbEnv;

#[derive(Parser)]
#[command(group = ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct ClearSendIdsArgs {
    #[arg(long, group = "input")]
    data_path: Option<String>,
    #[arg(long, group = "input")]
    network: Option<String>,
}

impl ClearSendIdsArgs {
    pub(crate) fn clear_send_ids(&self) {
        let path = get_path(&self.data_path, &self.network).join("wallets.ldb");

        let lmdb_env = LmdbEnv::new(&path).unwrap();
        lmdb_env.clear_database("clear_send_ids").unwrap();

        println!("{}", "Send IDs deleted");
    }
}
