use indexmap::IndexMap;
use rand::Rng;
use std::sync::{Arc, Mutex, RwLock, Weak};

use crate::{BlockHash, Vote};

pub(crate) struct VoteUniquer {
    votes: Mutex<IndexMap<BlockHash, Weak<RwLock<Vote>>>>,
}

impl VoteUniquer {
    const CLEANUP_COUNT: usize = 2;

    pub(crate) fn new() -> Self {
        Self {
            votes: Mutex::new(IndexMap::new()),
        }
    }

    pub(crate) fn size(&self) -> usize {
        self.votes.lock().unwrap().len()
    }

    pub(crate) fn unique(&self, original: &Arc<RwLock<Vote>>) -> Arc<RwLock<Vote>> {
        let key = original.read().unwrap().full_hash();
        let mut votes = self.votes.lock().unwrap();

        let result = match votes.get(&key) {
            Some(weak) => match weak.upgrade() {
                Some(b) => b,
                None => {
                    votes.insert(key, Arc::downgrade(original));
                    original.clone()
                }
            },
            None => {
                votes.insert(key, Arc::downgrade(original));
                original.clone()
            }
        };

        cleanup(votes);

        result
    }
}

fn cleanup(mut votes: std::sync::MutexGuard<IndexMap<BlockHash, Weak<RwLock<Vote>>>>) {
    let mut i = 0;
    while i < VoteUniquer::CLEANUP_COUNT && !votes.is_empty() {
        let random_offset = rand::thread_rng().gen_range(0..votes.len());
        let mut hash_to_remove = None;
        if let Some((hash, weak)) = votes.get_index(random_offset) {
            if weak.upgrade().is_none() {
                hash_to_remove = Some(*hash);
            }
        }
        if let Some(hash) = &hash_to_remove {
            votes.remove(hash);
        }
        i += 1;
    }
}
