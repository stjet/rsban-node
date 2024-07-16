use crate::cli::get_path;
use clap::{ArgGroup, Parser};
use rsnano_store_lmdb::LmdbStore;

#[derive(Parser)]
#[command(group = ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct PeerClearArgs {
    #[arg(long, group = "input")]
    data_path: Option<String>,
    #[arg(long, group = "input")]
    network: Option<String>,
}

impl PeerClearArgs {
    pub(crate) fn peer_clear(&self) {
        let path = get_path(&self.data_path, &self.network);
        let path = path.join("data.ldb");

        match LmdbStore::open_existing(&path) {
            Ok(store) => {
                let mut txn = store.tx_begin_write();
                store.peer.clear(&mut txn);
                println!("{}", "Database peers are removed");
            }
            Err(_) => {
                if self.data_path.is_some() {
                    println!("Database peers is not initialized in the given <data_path>. \nRun <daemon> or <initialize> command with the given <data_path> to initialize it.");
                } else if self.network.is_some() {
                    println!("Database peers is not initialized in the default path of the given <network>. \nRun <daemon> or <initialize> command with the given <network> to initialize it.");
                } else {
                    println!("Database peers is not initialized in the default path for the default network. \nRun <daemon> or <initialize> command to initialize it.");
                }
            }
        }
    }
}
