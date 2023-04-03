use crate::stats::{DetailType, Direction, StatType, Stats};
use rsnano_core::{BlockHash, HashOrAccount, UncheckedInfo, UncheckedKey};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, VecDeque},
    mem::size_of,
    ops::DerefMut,
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
};

const MEM_BLOCK_COUNT_MAX: usize = 64 * 1024;

struct UncheckedMapFlags {
    stopped: bool,
    active: bool,
}

pub struct UncheckedMap {
    join_handle: Option<JoinHandle<()>>,
    thread: Arc<UncheckedMapThread>,
    mutable: Arc<Mutex<ThreadMutableData>>,
    condition: Arc<Condvar>,
    stats: Arc<Stats>,
}

impl UncheckedMap {
    pub fn new(stats: Arc<Stats>, disable_delete: bool) -> Self {
        let mutable = Arc::new(Mutex::new(ThreadMutableData::new()));
        let condition = Arc::new(Condvar::new());

        let thread = Arc::new(UncheckedMapThread {
            disable_delete,
            mutable: mutable.clone(),
            condition: condition.clone(),
            stats: stats.clone(),
            back_buffer: Mutex::new(VecDeque::new()),
        });

        let thread_clone = thread.clone();
        let join_handle = std::thread::Builder::new()
            .name("Unchecked".to_string())
            .spawn(move || {
                thread_clone.run();
            })
            .unwrap();

        Self {
            join_handle: Some(join_handle),
            thread,
            mutable,
            condition,
            stats,
        }
    }

    pub fn stop(&mut self) {
        let mut lock = self.mutable.lock().unwrap();
        if !lock.stopped {
            lock.stopped = true;
            self.condition.notify_all();
        }
        drop(lock);
        if let Some(handle) = self.join_handle.take() {
            handle.join().unwrap();
        }
    }

    pub fn exists(&self, key: &UncheckedKey) -> bool {
        let lock = self.mutable.lock().unwrap();
        lock.entries_container.exists(key)
    }

    pub fn put(&self, dependency: HashOrAccount, info: UncheckedInfo) {
        let mut lock = self.mutable.lock().unwrap();
        let key = UncheckedKey::new(
            dependency.into(),
            info.block.clone().unwrap().read().unwrap().hash(),
        );
        let inserted = lock.entries_container.insert(Entry::new(key, info));
        if lock.entries_container.len() > MEM_BLOCK_COUNT_MAX {
            lock.entries_container.pop_front();
        }
        if inserted {
            self.stats
                .inc(StatType::Unchecked, DetailType::Put, Direction::In);
        }
    }

    pub fn get(&self, hash: &HashOrAccount) -> Vec<UncheckedInfo> {
        let lock = self.mutable.lock().unwrap();
        let mut result = Vec::new();
        lock.entries_container.for_each_with_dependency(
            hash,
            &mut |_, info| {
                result.push(info.clone());
            },
            &|| true,
        );
        result
    }

    pub fn clear(&self) {
        let mut lock = self.mutable.lock().unwrap();
        lock.entries_container.clear();
    }

    pub fn trigger(&self, dependency: &HashOrAccount) {
        let mut lock = self.mutable.lock().unwrap();
        lock.buffer.push_back(dependency.clone());
        drop(lock);
        self.stats
            .inc(StatType::Unchecked, DetailType::Trigger, Direction::In);
        self.condition.notify_all(); // Notify run ()
    }

    pub fn remove(&self, key: &UncheckedKey) {
        let mut lock = self.mutable.lock().unwrap();
        lock.entries_container.remove(key);
    }

    pub fn len(&self) -> usize {
        let lock = self.mutable.lock().unwrap();
        lock.entries_container.len()
    }

    pub fn entries_size() -> usize {
        EntriesContainer::entry_size()
    }

    pub fn buffer_count(&self) -> usize {
        let lock = self.mutable.lock().unwrap();
        lock.buffer.len()
    }

    pub fn buffer_entry_size() -> usize {
        size_of::<HashOrAccount>()
    }

    pub fn for_each(
        &self,
        action: Box<dyn FnMut(&UncheckedKey, &UncheckedInfo)>,
        predicate: Box<dyn Fn() -> bool>,
    ) {
        let lock = self.mutable.lock().unwrap();
        lock.entries_container.for_each(action, predicate)
    }

    pub fn for_each_with_dependency(
        &self,
        dependency: &HashOrAccount,
        action: &mut dyn FnMut(&UncheckedKey, &UncheckedInfo),
        predicate: &dyn Fn() -> bool,
    ) {
        let lock = self.mutable.lock().unwrap();
        lock.entries_container
            .for_each_with_dependency(dependency, action, predicate)
    }

    pub fn set_satisfied_observer(&self, callback: Box<dyn Fn(&UncheckedInfo) + Send>) {
        self.mutable.lock().unwrap().satisfied_callback = Some(callback);
    }
}

impl Drop for UncheckedMap {
    fn drop(&mut self) {
        self.stop()
    }
}

struct ThreadMutableData {
    stopped: bool,
    buffer: VecDeque<HashOrAccount>,
    writing_back_buffer: bool,
    entries_container: EntriesContainer,
    counter: u8,
    satisfied_callback: Option<Box<dyn Fn(&UncheckedInfo) + Send>>,
}

impl ThreadMutableData {
    fn new() -> Self {
        Self {
            stopped: false,
            buffer: VecDeque::new(),
            writing_back_buffer: false,
            entries_container: EntriesContainer::new(),
            counter: 0,
            satisfied_callback: None,
        }
    }
}

pub struct UncheckedMapThread {
    disable_delete: bool,
    mutable: Arc<Mutex<ThreadMutableData>>,
    condition: Arc<Condvar>,
    stats: Arc<Stats>,
    back_buffer: Mutex<VecDeque<HashOrAccount>>,
}

impl UncheckedMapThread {
    fn run(&self) {
        let mut lock = self.mutable.lock().unwrap();
        while !lock.stopped {
            if !lock.buffer.is_empty() {
                let mut back_buffer_lock = self.back_buffer.lock().unwrap();
                std::mem::swap(&mut lock.buffer, back_buffer_lock.deref_mut());
                lock.writing_back_buffer = true;
                drop(lock);
                self.process_queries(&back_buffer_lock);
                lock = self.mutable.lock().unwrap();
                lock.writing_back_buffer = false;
                back_buffer_lock.clear();
            } else {
                self.condition.notify_all();
                lock = self
                    .condition
                    .wait_while(lock, |other_lock| {
                        !other_lock.stopped && other_lock.buffer.is_empty()
                    })
                    .unwrap();
            }
        }
    }

    fn process_queries(&self, back_buffer: &VecDeque<HashOrAccount>) {
        for item in back_buffer {
            self.query_impl(item);
        }
    }

    pub fn query_impl(&self, hash: &HashOrAccount) {
        let mut delete_queue = Vec::new();
        let mut lock = self.mutable.lock().unwrap();
        lock.entries_container.for_each_with_dependency(
            hash,
            &mut |key, info| {
                delete_queue.push(key.clone());
                self.stats
                    .inc(StatType::Unchecked, DetailType::Satisfied, Direction::In);
                if let Some(callback) = &lock.satisfied_callback {
                    callback(info);
                }
            },
            &|| true,
        );
        if !self.disable_delete {
            for key in &delete_queue {
                lock.entries_container.remove(key);
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    key: UncheckedKey,
    info: UncheckedInfo,
}

impl Entry {
    fn new(key: UncheckedKey, info: UncheckedInfo) -> Self {
        Self { key, info }
    }
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.key.eq(&other.key)
    }
}

impl Eq for Entry {}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.key.partial_cmp(&other.key)
    }
}

impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key.cmp(&other.key)
    }
}

#[derive(Default, Clone, Debug)]
pub struct EntriesContainer {
    next_id: usize,
    by_key: BTreeMap<UncheckedKey, usize>,
    by_id: BTreeMap<usize, Entry>,
}

impl EntriesContainer {
    fn new() -> Self {
        Self {
            by_id: BTreeMap::new(),
            by_key: BTreeMap::new(),
            next_id: 0,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Entry> {
        self.by_id.values()
    }

    pub fn insert(&mut self, entry: Entry) -> bool {
        match self.by_key.get(&entry.key) {
            Some(_key) => false,
            None => {
                self.by_key.insert(entry.clone().key, self.next_id);

                self.by_id.insert(self.next_id, entry.clone());

                self.next_id += 1;

                true
            }
        }
    }

    fn is_empty(&self) -> bool {
        self.next_id == 0
    }

    fn remove(&mut self, key: &UncheckedKey) -> Option<Entry> {
        if let Some(id) = self.by_key.remove(key) {
            self.by_id.remove(&id)
        } else {
            None
        }
    }

    fn len(&self) -> usize {
        self.by_id.len()
    }

    fn pop_front(&mut self) {
        let entry = self.by_id.get(&(0)).unwrap().clone();
        self.by_id.pop_first();
        self.by_key.remove(&entry.key);
    }

    fn clear(&mut self) {
        self.by_id.clear();
        self.by_key.clear();
        self.next_id = 0;
    }

    fn exists(&self, key: &UncheckedKey) -> bool {
        self.by_key.contains_key(key)
    }

    fn entry_size() -> usize {
        size_of::<UncheckedKey>() + size_of::<Entry>() + size_of::<usize>() * 2
    }

    pub fn for_each(
        &self,
        mut action: Box<dyn FnMut(&UncheckedKey, &UncheckedInfo)>,
        predicate: Box<dyn Fn() -> bool>,
    ) {
        for entry in self.by_id.values() {
            if !predicate() {
                break;
            }
            action(&entry.key, &entry.info);
        }
    }

    pub fn for_each_with_dependency(
        &self,
        dependency: &HashOrAccount,
        action: &mut dyn FnMut(&UncheckedKey, &UncheckedInfo),
        predicate: &dyn Fn() -> bool,
    ) {
        let key = UncheckedKey::new(dependency.into(), BlockHash::zero());
        for (key, id) in self.by_key.range(key..) {
            if !predicate() || key.previous != dependency.into() {
                break;
            }
            let entry = self.by_id.get(id).unwrap();
            action(&entry.key, &entry.info);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_container() {
        let container = EntriesContainer::new();
        assert_eq!(container.next_id, 0);
        assert_eq!(container.by_id.len(), 0);
        assert_eq!(container.by_key.len(), 0);
    }

    #[test]
    fn insert_one_entry() {
        let mut container = EntriesContainer::new();

        let entry = test_entry(1);
        let new_insert = container.insert(entry.clone());

        assert_eq!(container.next_id, 1);
        assert_eq!(container.by_id.len(), 1);
        assert_eq!(container.by_id.get(&0).unwrap(), &entry);
        assert_eq!(container.by_key.len(), 1);
        assert_eq!(container.by_key.get(&entry.key).unwrap(), &0);
        assert_eq!(new_insert, true);
    }

    #[test]
    fn insert_two_entries_with_same_key() {
        let mut container = EntriesContainer::new();

        let entry = test_entry(1);
        let new_insert1 = container.insert(entry.clone());
        let new_insert2 = container.insert(entry);

        assert_eq!(container.next_id, 1);
        assert_eq!(container.by_id.len(), 1);
        assert_eq!(container.by_key.len(), 1);
        assert_eq!(new_insert1, true);
        assert_eq!(new_insert2, false);
    }

    #[test]
    fn insert_two_entries_with_different_key() {
        let mut container = EntriesContainer::new();

        let new_insert1 = container.insert(test_entry(1));
        let new_insert2 = container.insert(test_entry(2));

        assert_eq!(container.next_id, 2);
        assert_eq!(container.by_id.len(), 2);
        assert_eq!(container.by_key.len(), 2);
        assert_eq!(new_insert1, true);
        assert_eq!(new_insert2, true);
    }

    #[test]
    fn pop_front() {
        let mut container = EntriesContainer::new();

        container.insert(test_entry(1));
        let entry = test_entry(2);
        container.insert(entry.clone());

        container.pop_front();

        assert_eq!(container.next_id, 2);
        assert_eq!(container.by_id.len(), 1);
        assert_eq!(container.by_id.get(&1).is_some(), true);
        assert_eq!(container.by_key.len(), 1);
        assert_eq!(container.by_key.get(&entry.key).unwrap(), &1);
        assert_eq!(container.len(), 1);
    }

    #[test]
    fn remove_by_key() {
        let mut container = EntriesContainer::new();
        container.insert(test_entry(1));
        let entry = test_entry(2);
        container.insert(entry.clone());

        container.remove(&entry.key);

        assert_eq!(container.len(), 1);
        assert_eq!(container.by_id.len(), 1);
        assert_eq!(container.by_key.len(), 1);
        assert_eq!(container.exists(&entry.key), false);
    }

    fn test_entry<T: Into<BlockHash>>(hash: T) -> Entry {
        Entry::new(
            UncheckedKey::new(hash.into(), BlockHash::default()),
            UncheckedInfo::default(),
        )
    }
}
