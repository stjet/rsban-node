use std::{
    collections::{HashMap, HashSet, VecDeque},
    mem::size_of,
    sync::{Arc, Mutex, RwLock},
};

use super::Representative;
use crate::{transport::ChannelEnum, voting::Vote};
use rsnano_core::{
    utils::{ContainerInfo, ContainerInfoComponent},
    Account, BlockHash,
};

pub struct RepCrawler {
    /// Probable representatives
    probable_reps: Mutex<HashMap<Account, Representative>>,
    data: Mutex<RepCrawlerData>,
}

struct RepCrawlerData {
    /** We have solicted votes for these random blocks */
    active: HashSet<BlockHash>,
    responses: VecDeque<(Arc<ChannelEnum>, Arc<RwLock<Vote>>)>,
}

impl RepCrawlerData {
    fn new() -> Self {
        Self {
            active: HashSet::new(),
            responses: VecDeque::new(),
        }
    }
}

impl RepCrawler {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(RepCrawlerData::new()),
            probable_reps: Mutex::new(HashMap::new()),
        }
    }

    pub fn add_rep(&self, rep: Representative) {
        let mut guard = self.probable_reps.lock().unwrap();
        guard.insert(rep.account().clone(), rep); // todo panic if already added
    }

    pub fn remove(&self, hash: &BlockHash) {
        let mut guard = self.data.lock().unwrap();
        guard.active.remove(hash);
    }

    pub fn active_contains(&self, hash: &BlockHash) -> bool {
        let guard = self.data.lock().unwrap();
        guard.active.contains(hash)
    }

    pub fn insert_active(&self, hash: BlockHash) {
        let mut guard = self.data.lock().unwrap();
        guard.active.insert(hash);
    }

    pub fn insert_response(&self, channel: Arc<ChannelEnum>, vote: Arc<RwLock<Vote>>) {
        let mut guard = self.data.lock().unwrap();
        guard.responses.push_back((channel, vote));
    }

    pub fn clear_responses(&self) {
        let mut guard = self.data.lock().unwrap();
        guard.responses.clear();
    }
    pub fn response(&self, channel: Arc<ChannelEnum>, vote: Arc<Vote>, force: bool) {
        // let guard = self.data.lock().unwrap();
        // for hash in &vote.hashes{
        //     if force ||
        // }
    }

    pub fn collect_container_info(&self, name: String) -> ContainerInfoComponent {
        let guard = self.data.lock().unwrap();
        ContainerInfoComponent::Composite(
            name,
            vec![ContainerInfoComponent::Leaf(ContainerInfo {
                name: "active".to_string(),
                count: guard.active.len(),
                sizeof_element: size_of::<Account>() + size_of::<Representative>(),
            })],
        )
    }
}
