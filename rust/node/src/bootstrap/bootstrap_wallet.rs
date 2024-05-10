use std::{
    collections::VecDeque,
    sync::{atomic::Ordering, Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use rsnano_core::Account;
use tracing::{debug, info};

use crate::utils::ThreadPool;

use super::{BootstrapAttempt, BootstrapConnections, BootstrapConnectionsExt};

pub struct BootstrapAttemptWallet {
    pub attempt: BootstrapAttempt,
    mutex: Mutex<WalletData>,
    connections: Arc<BootstrapConnections>,
    workers: Arc<dyn ThreadPool>,
}

impl BootstrapAttemptWallet {
    pub fn requeue_pending(&self, account: Account) {
        {
            let mut guard = self.mutex.lock().unwrap();
            guard.wallet_accounts.push_front(account);
        }
        self.attempt.condition.notify_all();
    }

    pub fn run(&self) {
        debug_assert!(self.attempt.started.load(Ordering::SeqCst));
        self.connections.populate_connections(false);
        let start_time = Instant::now();
        let max_time = Duration::from_secs(60 * 10);
        let mut guard = self.mutex.lock().unwrap();
        while self.wallet_finished(&guard) && start_time.elapsed() < max_time {
            if !guard.wallet_accounts.is_empty() {
                guard = request_pending(guard);
            } else {
                guard = self
                    .attempt
                    .condition
                    .wait_timeout(guard, Duration::from_millis(1000))
                    .unwrap()
                    .0;
            }
        }
        if !self.attempt.stopped() {
            info!("Completed wallet lazy pulls");
        }
        drop(guard);
        self.stop();
        self.attempt.condition.notify_all();
    }

    fn wallet_finished(&self, data: &WalletData) -> bool {
        let running = !self.attempt.stopped.load(Ordering::SeqCst);
        let more_accounts = !data.wallet_accounts.is_empty();
        let still_pulling = self.attempt.pulling.load(Ordering::SeqCst) > 0;
        return running && (more_accounts || still_pulling);
    }

    fn request_pending<'a>(
        &'a self,
        mut guard: MutexGuard<'a, WalletData>,
    ) -> MutexGuard<'a, WalletData> {
        drop(guard);
        let (connection_l, should_stop) = self.connections.connection(false);
        if should_stop {
            debug!("Bootstrap attempt stopped because there are no peers");
            self.stop();
        }

        let mut guard = self.mutex.lock().unwrap();
        if connection_l.is_some() && !self.attempt.stopped() {
            let account = guard.wallet_accounts.pop_front().unwrap();
            self.attempt.pulling.fetch_add(1, Ordering::SeqCst);
            //	auto this_l = std::dynamic_pointer_cast<nano::bootstrap_attempt_wallet> (shared_from_this ());
            self.workers.push_task(Box::new(move || {
                let client = BulkPullAccountClient::new()
            }));
            // The bulk_pull_account_client destructor attempt to requeue_pull which can cause a deadlock if this is the last reference
            // Dispatch request in an external thread in case it needs to be destroyed
            self.w
            //	node->background ([connection_l, this_l, account, node] () {
            //		auto client (std::make_shared<nano::bulk_pull_account_client> (node, connection_l, this_l, account));
            //		client->request ();
            //	});
        }
        guard
    }
}

struct WalletData {
    wallet_accounts: VecDeque<Account>,
}
