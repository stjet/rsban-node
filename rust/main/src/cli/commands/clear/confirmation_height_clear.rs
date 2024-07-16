use crate::cli::get_path;
use clap::{ArgGroup, Parser};
use rsnano_core::{Account, ConfirmationHeightInfo, Networks};
use rsnano_ledger::LedgerConstants;
use rsnano_node::config::NetworkConstants;
use rsnano_store_lmdb::LmdbStore;

#[derive(Parser)]
#[command(group = ArgGroup::new("input1")
    .args(&["account", "all"]))]
#[command(group = ArgGroup::new("input2")
    .args(&["data_path", "network"]))]
pub(crate) struct ConfirmationHeightClearArgs {
    #[arg(long, group = "input1")]
    account: Option<String>,
    #[arg(long, group = "input1")]
    all: bool,
    #[arg(long, group = "input2")]
    data_path: Option<String>,
    #[arg(long, group = "input2")]
    network: Option<String>,
}

impl ConfirmationHeightClearArgs {
    pub(crate) fn confirmation_height_clear(&self) {
        let path = get_path(&self.data_path, &self.network);
        let path = path.join("data.ldb");

        let genesis_block = match NetworkConstants::active_network() {
            Networks::NanoDevNetwork => LedgerConstants::dev().genesis,
            Networks::NanoBetaNetwork => LedgerConstants::beta().genesis,
            Networks::NanoLiveNetwork => LedgerConstants::live().genesis,
            Networks::NanoTestNetwork => LedgerConstants::test().genesis,
            Networks::Invalid => panic!("This should not happen!"),
        };

        let genesis_account = genesis_block.account();
        let genesis_hash = genesis_block.hash();

        match LmdbStore::open_existing(&path) {
            Ok(store) => {
                let mut txn = store.tx_begin_write();
                store.confirmation_height.clear(&mut txn);
                if let Some(account_hex) = &self.account {
                    match Account::decode_hex(account_hex) {
                        Ok(account) => {
                            let mut conf_height_reset_num = 0;
                            let mut info = store.confirmation_height.get(&txn, &account).unwrap();
                            if account == genesis_account {
                                conf_height_reset_num += 1;
                                info.height = conf_height_reset_num;
                                info.frontier = genesis_hash;
                                store.confirmation_height.put(&mut txn, &account, &info);
                            } else {
                                store.confirmation_height.del(&mut txn, &account);
                            }
                            println!(
                                "Confirmation height of account {:?} is set to {:?}",
                                account_hex, conf_height_reset_num
                            );
                        }
                        Err(_) => {
                            println!("Invalid account");
                        }
                    }
                } else if self.all {
                    store.confirmation_height.clear(&mut txn);
                    store.confirmation_height.put(
                        &mut txn,
                        &genesis_account,
                        &ConfirmationHeightInfo::new(1, genesis_hash),
                    );
                    println!("Confirmation heights of all accounts (except genesis which is set to 1) are set to 0");
                } else {
                    println!("Specify either valid account id or 'all'");
                }
            }
            Err(_) => {
                if self.data_path.is_some() {
                    println!("Database confirmation height is not initialized in the given <data_path>. \nRun <daemon> or <initialize> command with the given <data_path> to initialize it.");
                } else if self.network.is_some() {
                    println!("Database confirmation height is not initialized in the default path of the given <network>. \nRun <daemon> or <initialize> command with the given <network> to initialize it.");
                } else {
                    println!("Database confirmation height is not initialized in the default path for the default network. \nRun <daemon> or <initialize> command to initialize it.");
                }
            }
        }
    }
}
