use super::BlockProcessor;
use crate::{
    representatives::RepresentativeRegister,
    stats::{DetailType, Direction, StatType, Stats},
    transport::{BandwidthLimiter, BufferDropPolicy, ChannelEnum, TcpChannels, TrafficType},
};
use rsnano_core::{
    utils::{ContainerInfo, ContainerInfoComponent},
    BlockEnum, BlockHash,
};
use rsnano_ledger::Ledger;
use rsnano_messages::{Message, Publish};
use rsnano_store_lmdb::Transaction;
use std::{
    collections::{HashMap, VecDeque},
    mem::size_of,
    sync::{Arc, Condvar, Mutex, MutexGuard},
    thread::JoinHandle,
    time::{Duration, Instant},
};

///  Broadcasts blocks to the network
/// Tracks local blocks for more aggressive propagation
pub struct LocalBlockBroadcaster {
    block_processor: Arc<BlockProcessor>,
    stats: Arc<Stats>,
    ledger: Arc<Ledger>,
    confirming_set: Arc<Ledger>,
    thread: Mutex<Option<JoinHandle<()>>>,
    enabled: bool,
    mutex: Mutex<LocalBlockBroadcasterData>,
    condition: Condvar,
    limiter: BandwidthLimiter,
    channels: Arc<TcpChannels>,
    representatives: Arc<Mutex<RepresentativeRegister>>,
}

impl LocalBlockBroadcaster {
    const MAX_SIZE: usize = 1024 * 8;
    const CHECK_INTERVAL: Duration = Duration::from_secs(30);
    const BROADCAST_INTERVAL: Duration = Duration::from_secs(60);
    const BROADCAST_RATE_LIMIT: usize = 32;
    const BROADCAST_RATE_BURST_RATIO: f64 = 3.0;

    pub fn new(
        block_processor: Arc<BlockProcessor>,
        stats: Arc<Stats>,
        channels: Arc<TcpChannels>,
        representatives: Arc<Mutex<RepresentativeRegister>>,
        ledger: Arc<Ledger>,
        confirming_set: Arc<Ledger>,
        enabled: bool,
    ) -> Self {
        Self {
            block_processor,
            stats,
            channels,
            ledger,
            confirming_set,
            representatives,
            thread: Mutex::new(None),
            enabled,
            mutex: Mutex::new(LocalBlockBroadcasterData {
                stopped: false,
                local_blocks: Default::default(),
            }),
            condition: Condvar::new(),
            limiter: BandwidthLimiter::new(
                Self::BROADCAST_RATE_BURST_RATIO,
                Self::BROADCAST_RATE_LIMIT,
            ),
        }
    }

    pub fn stop(&self) {
        self.mutex.lock().unwrap().stopped = true;
        self.condition.notify_all();
        if let Some(handle) = self.thread.lock().unwrap().take() {
            handle.join().unwrap();
        }
    }

    fn run(&self) {
        let mut guard = self.mutex.lock().unwrap();
        while !guard.stopped {
            self.stats
                .inc(StatType::LocalBlockBroadcaster, DetailType::Loop);
            guard = self.condition.wait_while(guard, |g| !g.stopped).unwrap();
            if !guard.stopped {
                self.cleanup(&mut guard);
                guard = self.run_broadcasts(guard);
            }
        }
    }

    fn run_broadcasts<'a>(
        &'a self,
        mut guard: MutexGuard<'a, LocalBlockBroadcasterData>,
    ) -> MutexGuard<'a, LocalBlockBroadcasterData> {
        let mut to_broadcast = Vec::new();
        let now = Instant::now();
        guard.local_blocks.modify(|entry| {
            if entry.last_broadcast.is_none()
                || entry.last_broadcast.unwrap().elapsed() >= Self::BROADCAST_INTERVAL
            {
                entry.last_broadcast = Some(now);
                to_broadcast.push(Arc::clone(&entry.block));
            }
        });
        drop(guard);

        for block in to_broadcast {
            while !self.limiter.should_pass(1) {
                guard = self.mutex.lock().unwrap();
                drop(
                    self.condition
                        .wait_timeout_while(guard, Duration::from_millis(100), |g| !g.stopped)
                        .unwrap(),
                );
            }

            self.stats.inc_dir(
                StatType::LocalBlockBroadcaster,
                DetailType::Broadcast,
                Direction::Out,
            );

            self.flood_block_initial((*block).clone());
        }

        self.mutex.lock().unwrap()
    }

    fn cleanup(&self, data: &mut LocalBlockBroadcasterData) {
        // Erase oldest blocks if the queue gets too big
        while data.local_blocks.len() > Self::MAX_SIZE {
            self.stats
                .inc(StatType::LocalBlockBroadcaster, DetailType::EraseOldest);
            data.local_blocks.pop_front();
        }

        // TODO: Mutex is held during IO, but it should be fine since it's not performance critical
        let mut tx = self.ledger.read_txn();
        data.local_blocks.retain(|entry| {
            tx.refresh_if_needed(Duration::from_millis(500));

            if entry.last_broadcast.is_none() {
                // This block has never been broadcasted, keep it so it's broadcasted at least once
                return true;
            }
            if self.confirming_set.block_exists(&tx, &entry.block.hash())
                || self.ledger.block_confirmed(&tx, &entry.block.hash())
            {
                self.stats
                    .inc(StatType::LocalBlockBroadcaster, DetailType::EraseConfirmed);
                return false;
            }
            true
        });
    }

    pub fn collect_container_info(&self, name: impl Into<String>) -> ContainerInfoComponent {
        let guard = self.mutex.lock().unwrap();
        ContainerInfoComponent::Composite(
            name.into(),
            vec![ContainerInfoComponent::Leaf(ContainerInfo {
                name: "local".to_string(),
                count: guard.local_blocks.len(),
                sizeof_element: OrderedLocals::ELEMENT_SIZE,
            })],
        )
    }

    fn flood_block_initial(&self, block: BlockEnum) {
        let message = Message::Publish(Publish::new(block));
        for i in self
            .representatives
            .lock()
            .unwrap()
            .principal_representatives()
        {
            i.channel.send(
                &message,
                None,
                BufferDropPolicy::NoLimiterDrop,
                TrafficType::Generic,
            )
        }

        for i in self.list_no_pr(self.channels.fanout(1.0)) {
            i.send(
                &message,
                None,
                BufferDropPolicy::NoLimiterDrop,
                TrafficType::Generic,
            )
        }
    }

    fn list_no_pr(&self, count: usize) -> Vec<Arc<ChannelEnum>> {
        let mut channels = self.channels.random_list(usize::MAX, 0, true);
        {
            let guard = self.representatives.lock().unwrap();
            channels.retain(|c| !guard.is_pr(c));
        }
        channels.truncate(count);
        channels
    }
}

impl Drop for LocalBlockBroadcaster {
    fn drop(&mut self) {
        // Thread must be stopped before destruction
        debug_assert!(self.thread.lock().unwrap().is_none())
    }
}

pub trait LocalBlockBroadcasterExt {
    fn start(&self);
}

impl LocalBlockBroadcasterExt for Arc<LocalBlockBroadcaster> {
    fn start(&self) {
        if !self.enabled {
            return;
        }

        debug_assert!(self.thread.lock().unwrap().is_none());
        let self_l = Arc::clone(self);
        *self.thread.lock().unwrap() = Some(
            std::thread::Builder::new()
                .name("Local broadcast".to_string())
                .spawn(move || self_l.run())
                .unwrap(),
        );
    }
}

struct LocalBlockBroadcasterData {
    stopped: bool,
    local_blocks: OrderedLocals,
}

enum BroadcastStrategy {
    Normal,
    Aggressive,
}

struct LocalEntry {
    block: Arc<BlockEnum>,
    arrival: Instant,
    last_broadcast: Option<Instant>,
}

#[derive(Default)]
struct OrderedLocals {
    by_hash: HashMap<BlockHash, LocalEntry>,
    sequenced: VecDeque<BlockHash>,
}

impl OrderedLocals {
    pub const ELEMENT_SIZE: usize = size_of::<LocalEntry>() + size_of::<BlockHash>() * 2;
    fn len(&self) -> usize {
        self.sequenced.len()
    }

    fn insert(&mut self, entry: LocalEntry) {
        let hash = entry.block.hash();
        if let Some(old) = self.by_hash.insert(entry.block.hash(), entry) {
            self.sequenced.retain(|i| *i != old.block.hash());
        }
        self.sequenced.push_back(hash);
    }

    fn modify(&mut self, mut f: impl FnMut(&mut LocalEntry)) {
        for hash in &self.sequenced {
            if let Some(entry) = self.by_hash.get_mut(hash) {
                f(entry);
            }
        }
    }

    fn pop_front(&mut self) -> Option<LocalEntry> {
        let hash = self.sequenced.pop_front()?;
        self.by_hash.remove(&hash)
    }

    fn retain(&mut self, mut f: impl FnMut(&LocalEntry) -> bool) {
        self.by_hash.retain(|_, v| {
            let retain = f(v);
            if !retain {
                self.sequenced.retain(|i| *i != v.block.hash())
            }
            retain
        });
    }
}
