#[cfg(test)]
use mock_instant::Instant;
#[cfg(not(test))]
use std::time::Instant;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    time::Duration,
};

use crate::{BlockHash, Root};

pub struct VoteSpacing {
    delay: Duration,
    recent: EntryContainer,
}

impl VoteSpacing {
    pub fn new(delay: Duration) -> Self {
        Self {
            recent: EntryContainer::new(),
            delay,
        }
    }

    pub fn votable(&self, root: &Root, hash: &BlockHash) -> bool {
        for item in self.recent.by_root(root) {
            if *hash != item.hash && item.time.elapsed() < self.delay {
                return false;
            }
        }

        true
    }

    pub fn flag(&mut self, root: &Root, hash: &BlockHash) {
        self.trim();
        let time = Instant::now();
        if !self.recent.change_time_for_root(root, time) {
            self.recent.insert(Entry {
                root: *root,
                hash: *hash,
                time,
            });
        }
    }

    pub fn len(&self) -> usize {
        self.recent.len()
    }

    fn trim(&mut self) {
        self.recent.trim(self.delay);
    }
}

struct Entry {
    root: Root,
    hash: BlockHash,
    time: Instant,
}

#[derive(Default)]
struct EntryContainer {
    entries: HashMap<usize, Entry>,
    by_root: HashMap<Root, HashSet<usize>>,
    by_time: BTreeMap<Instant, HashSet<usize>>,
    next_id: usize,
    empty_id_set: HashSet<usize>,
}

impl EntryContainer {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn insert(&mut self, entry: Entry) {
        let id = self.next_id;
        self.next_id += 1;

        let by_root = self.by_root.entry(entry.root).or_default();
        by_root.insert(id);

        let by_time = self.by_time.entry(entry.time).or_default();
        by_time.insert(id);

        self.entries.insert(id, entry);
    }

    pub fn by_root(&self, root: &Root) -> impl Iterator<Item = &Entry> + '_ {
        match self.by_root.get(root) {
            Some(ids) => self.iter_entries(ids),
            None => self.iter_entries(&self.empty_id_set),
        }
    }

    fn iter_entries<'a>(&'a self, ids: &'a HashSet<usize>) -> impl Iterator<Item = &Entry> + 'a {
        ids.iter().map(|&id| &self.entries[&id])
    }

    fn trim(&mut self, upper_bound: Duration) {
        let mut instants_to_remove = Vec::new();
        for (&instant, ids) in self.by_time.iter() {
            if instant.elapsed() < upper_bound {
                break;
            }

            instants_to_remove.push(instant);

            for id in ids {
                let entry = self.entries.remove(id).unwrap();

                let by_root = self.by_root.get_mut(&entry.root).unwrap();
                by_root.remove(id);
            }
        }

        for instant in instants_to_remove {
            self.by_time.remove(&instant);
        }
    }

    fn change_time_for_root(&mut self, root: &Root, time: Instant) -> bool {
        match self.by_root.get(root) {
            Some(ids) => {
                for id in ids {
                    if let Some(entry) = self.entries.get_mut(id) {
                        let old_time = entry.time;
                        entry.time = time;
                        if let Some(time_ids) = self.by_time.get_mut(&old_time) {
                            time_ids.remove(id);
                            if time_ids.is_empty() {
                                self.by_time.remove(&old_time);
                            }
                        }
                        self.by_time.entry(time).or_default().insert(*id);
                    }
                }
                true
            }
            None => false,
        }
    }

    fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use mock_instant::MockClock;

    use super::*;

    #[test]
    fn basic() {
        let mut spacing = VoteSpacing::new(Duration::from_millis(100));
        let root1 = Root::from(1);
        let root2 = Root::from(2);
        let hash1 = BlockHash::from(3);
        let hash2 = BlockHash::from(4);
        let hash3 = BlockHash::from(5);

        assert_eq!(spacing.len(), 0);
        assert_eq!(spacing.votable(&root1, &hash1), true);

        spacing.flag(&root1, &hash1);
        assert_eq!(spacing.len(), 1);
        assert_eq!(spacing.votable(&root1, &hash1), true);
        assert_eq!(spacing.votable(&root1, &hash2), false);

        spacing.flag(&root2, &hash3);
        assert_eq!(spacing.len(), 2);
    }

    #[test]
    fn prune() {
        let length = Duration::from_millis(100);
        let mut spacing = VoteSpacing::new(length);
        spacing.flag(&Root::from(1), &BlockHash::from(3));
        assert_eq!(spacing.len(), 1);

        MockClock::advance(length);
        spacing.flag(&Root::from(2), &BlockHash::from(4));
        assert_eq!(spacing.len(), 1);
    }
}
