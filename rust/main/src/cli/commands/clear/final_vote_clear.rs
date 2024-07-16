use crate::cli::get_path;
use clap::{ArgGroup, Parser};
use rsnano_core::Root;
use rsnano_store_lmdb::LmdbStore;

#[derive(Parser)]
#[command(group = ArgGroup::new("input1")
    .args(&["root", "all"]))]
#[command(group = ArgGroup::new("input2")
    .args(&["data_path", "network"]))]
pub(crate) struct FinalVoteClearArgs {
    #[arg(long, group = "input1")]
    root: Option<String>,
    #[arg(long, group = "input1")]
    all: bool,
    #[arg(long, group = "input2")]
    data_path: Option<String>,
    #[arg(long, group = "input2")]
    network: Option<String>,
}

impl FinalVoteClearArgs {
    pub(crate) fn final_vote_clear(&self) -> anyhow::Result<()> {
        let path = get_path(&self.data_path, &self.network);
        let path = path.join("data.ldb");

        match LmdbStore::open_existing(&path) {
            Ok(store) => {
                let mut txn = store.tx_begin_write();
                store.online_weight.clear(&mut txn);
                if let Some(root) = &self.root {
                    match Root::decode_hex(root) {
                        Ok(root_decoded) => {
                            store.final_vote.del(&mut txn, &root_decoded);
                            println!("Successfully cleared final votes");
                        }
                        Err(_) => {
                            println!("Invalid root");
                        }
                    }
                } else if self.all {
                    store.final_vote.clear(&mut txn);
                    println!("All final votes are cleared");
                } else {
                    println!("Either specify a single --root to clear or --all to clear all final votes (not recommended)");
                }
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

        Ok(())
    }
}
