use crate::cli::get_path;
use clap::{ArgGroup, Parser};
use rsnano_store_lmdb::LmdbStore;

#[derive(Parser)]
#[command(group = ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct OnlineWeightArgs {
    #[arg(long, group = "input")]
    data_path: Option<String>,
    #[arg(long, group = "input")]
    network: Option<String>,
}

impl OnlineWeightArgs {
    pub(crate) fn online_weight(&self) {
        let path = get_path(&self.data_path, &self.network).join("data.ldb");

        match LmdbStore::open_existing(&path) {
            Ok(store) => {
                let mut txn = store.tx_begin_write();
                store.online_weight.clear(&mut txn);
                println!("{}", "Online weight records are removed");
            }
            Err(_) => {
                if self.data_path.is_some() {
                    println!("Database online weight is not initialized in the given <data_path>. \nRun <daemon> or <initialize> command with the given <data_path> to initialize it.");
                } else if self.network.is_some() {
                    println!("Database online weight is not initialized in the default path of the given <network>. \nRun <daemon> or <initialize> command with the given <network> to initialize it.");
                } else {
                    println!("Database online weight is not initialized in the default path for the default network. \nRun <daemon> or <initialize> command to initialize it.");
                }
            }
        }
    }
}
