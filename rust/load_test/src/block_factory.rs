use std::{sync::{Arc, atomic::{AtomicUsize, Ordering}}, io::Write, time::Duration, collections::HashMap};
use crate::{Account, RpcClient, AccountInfo};
use anyhow::Result;
use rand::Rng;
use rsnano::secure::DEV_GENESIS;
use tokio::{time::sleep, sync::Semaphore, spawn};

pub struct BlockFactory {
    send_count: usize,
    simultaneous_process_calls: usize,
    send_calls_remaining: Arc<AtomicUsize>,
    destination_accounts: Arc<Vec<Account>>,
    wallet: Arc<String>,
    node_client: Arc<RpcClient>,
}

impl BlockFactory {
    // Creates the specified amount of send and receive blocks
    pub async fn create_blocks(
        send_count: usize,
        simultaneous_process_calls: usize,
        destination_accounts: Arc<Vec<Account>>,
        wallet: Arc<String>,
        node_client: Arc<RpcClient>,
    ) -> Result<HashMap<String, AccountInfo>> {
        let factory = Arc::new(Self {
            send_count,
            simultaneous_process_calls,
            send_calls_remaining: Arc::new(AtomicUsize::new(send_count)),
            destination_accounts,
            wallet,
            node_client,
        });

        let f1 = factory.clone();
        let f2 = factory.clone();
        let send_loop = spawn(async move { f1.send_loop().await });
        let progress_loop = spawn(async move { f2.show_send_progress().await });

        let (send_result, wait_result) = tokio::join!(send_loop, progress_loop);
        send_result??;
        wait_result??;

        factory.get_destination_accounts_info().await
    }

    async fn get_destination_accounts_info(&self) -> Result<HashMap<String, AccountInfo>>{
        let mut known_account_info = HashMap::new();
        for i in 0..self.destination_accounts.len() {
            known_account_info.insert(
                self.destination_accounts[i].as_string.clone(),
                self.node_client
                    .account_info_rpc(&self.destination_accounts[i].as_string)
                    .await?,
            );
        }
        Ok(known_account_info)
    }

    async fn show_send_progress(&self) -> Result<()> {
        print!("\rPrimary node processing transactions: 00%");
        std::io::stdout().flush()?;

        let mut last_percent = 0;
        while self.send_calls_remaining.load(Ordering::SeqCst) != 0 {
            let percent = (100_f64
                * ((self.send_count as f64
                    - self.send_calls_remaining.load(Ordering::SeqCst) as f64)
                    / (self.send_count as f64))) as i32;
            if last_percent != percent {
                print!("\rPrimary node processing transactions: {:02}%", percent,);
                std::io::stdout().flush()?;
                last_percent = percent;
                sleep(Duration::from_millis(100)).await;
            }
        }
        println!("\rPrimary node processed transactions                ");
        Ok(())
    }

    async fn send_loop(&self) -> Result<()> {
        let sem = Arc::new(Semaphore::new(self.simultaneous_process_calls));
        let mut join_handles = Vec::new();
        for i in 0..self.send_count {
            // Send from genesis account to different accounts and receive the funds

            let permit = Arc::clone(&sem).acquire_owned().await?;
            let wallet = self.wallet.clone();
            let send_calls_remaining = self.send_calls_remaining.clone();
            let destination_accounts = self.destination_accounts.clone();
            let node_client = self.node_client.clone();

            let handle = spawn(async move {
                let _permit = permit;
                let destination_account = if i < destination_accounts.len() {
                    &destination_accounts[i]
                } else {
                    let random_account_index =
                        rand::thread_rng().gen_range(0..destination_accounts.len());
                    &destination_accounts[random_account_index]
                };

                let genesis_account = DEV_GENESIS.as_block().account().encode_account();

                let res = node_client
                    .send_receive(&wallet, &genesis_account, &destination_account.as_string)
                    .await;
                send_calls_remaining.fetch_sub(1, Ordering::SeqCst);
                res
            });
            join_handles.push(handle);
        }

        for h in join_handles {
            h.await??;
        }
        Ok(())
    }
}
