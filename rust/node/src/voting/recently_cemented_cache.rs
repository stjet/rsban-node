use super::ElectionStatus;
use std::{collections::VecDeque, mem::size_of, sync::Mutex};

pub struct RecentlyCementedCache {
    cemented: Mutex<VecDeque<ElectionStatus>>,
    max_size: usize,
}

impl RecentlyCementedCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            cemented: Mutex::new(VecDeque::new()),
            max_size,
        }
    }

    pub fn get_cemented(&self) -> VecDeque<ElectionStatus> {
        self.cemented.lock().unwrap().clone()
    }

    pub fn put(&self, election_status: ElectionStatus) {
        let mut cemented = self.cemented.lock().unwrap();
        cemented.push_back(election_status);
        if cemented.len() > self.max_size {
            cemented.pop_front();
        }
    }

    pub fn size(&self) -> usize {
        self.cemented.lock().unwrap().len()
    }

    pub fn element_size() -> usize {
        size_of::<ElectionStatus>()
    }
}
