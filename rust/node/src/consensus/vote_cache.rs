use crate::stats::{DetailType, Direction, StatType, Stats};
use rsnano_core::{
    utils::{ContainerInfo, ContainerInfoComponent, TomlWriter},
    Account, Amount, BlockHash, Vote,
};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap},
    fmt::Debug,
    mem::size_of,
    sync::Arc,
    time::Duration,
};

#[cfg(test)]
use mock_instant::Instant;
#[cfg(not(test))]
use std::time::Instant;

pub struct VoteCacheConfig {
    pub max_size: usize,
    pub max_voters: usize,
    pub age_cutoff: Duration,
}

impl VoteCacheConfig {
    pub fn serialize_toml(&self, toml: &mut dyn TomlWriter) -> anyhow::Result<()> {
        toml.put_usize(
            "max_size",
            self.max_size,
            "Maximum number of blocks to cache votes for. \ntype:uint64",
        )?;

        toml.put_usize(
            "max_voters",
            self.max_voters,
            "Maximum number of voters to cache per block. \ntype:uint64",
        )?;

        toml.put_u64(
            "age_cutoff",
            self.age_cutoff.as_secs(),
            "Maximum age of votes to keep in cache. \ntype:seconds",
        )
    }
}

impl Default for VoteCacheConfig {
    fn default() -> Self {
        Self {
            max_size: 1024 * 128,
            max_voters: 128,
            age_cutoff: Duration::from_secs(5 * 60),
        }
    }
}

///	A container holding votes that do not match any active or recently finished elections.
///	It keeps track of votes in two internal structures: cache and queue
///
///	Cache: Stores votes associated with a particular block hash with a bounded maximum number of votes per hash.
///			When cache size exceeds `max_size` oldest entries are evicted first.
pub struct VoteCache {
    config: VoteCacheConfig,
    cache: CacheEntryCollection,
    next_id: usize,
    last_cleanup: Instant,
    stats: Arc<Stats>,
}

impl VoteCache {
    pub fn new(config: VoteCacheConfig, stats: Arc<Stats>) -> Self {
        VoteCache {
            last_cleanup: Instant::now(),
            config,
            cache: CacheEntryCollection::default(),
            next_id: 0,
            stats,
        }
    }

    pub fn vote(&mut self, hash: &BlockHash, vote: &Vote, rep_weight: Amount) {
        /*
         * If there is no cache entry for the block hash, create a new entry for both cache and queue.
         * Otherwise update existing cache entry and, if queue contains entry for the block hash, update the queue entry
         */
        let cache_entry_exists = self.cache.modify_by_hash(hash, |existing| {
            existing.vote(
                &vote.voting_account,
                vote.timestamp(),
                rep_weight,
                self.config.max_voters,
            );
        });

        if cache_entry_exists {
            self.stats
                .inc(StatType::VoteCache, DetailType::Update, Direction::In)
        } else {
            self.stats
                .inc(StatType::VoteCache, DetailType::Insert, Direction::In);
            let id = self.next_id;
            self.next_id += 1;
            let mut cache_entry = CacheEntry::new(id, *hash);
            cache_entry.vote(
                &vote.voting_account,
                vote.timestamp(),
                rep_weight,
                self.config.max_voters,
            );

            self.cache.insert(cache_entry);

            // When cache overflown remove the oldest entry
            if self.cache.len() > self.config.max_size {
                self.cache.pop_front();
            }
        }
    }

    pub fn empty(&self) -> bool {
        self.cache.is_empty()
    }

    pub fn size(&self) -> usize {
        self.cache.len()
    }

    /// Tries to find an entry associated with block hash
    pub fn find(&self, hash: &BlockHash) -> Option<&CacheEntry> {
        self.cache.get_by_hash(hash)
    }

    /// Removes an entry associated with block hash, does nothing if entry does not exist
    /// return true if hash existed and was erased, false otherwise
    pub fn erase(&mut self, hash: &BlockHash) -> bool {
        self.cache.remove_by_hash(hash).is_some()
    }

    /// Returns blocks with highest observed tally, greater than `min_tally`
    /// The blocks are sorted in descending order by final tally, then by tally
    /// @param min_tally minimum tally threshold, entries below with their voting weight
    /// below this will be ignore
    pub fn top(&mut self, min_tally: impl Into<Amount>) -> Vec<TopEntry> {
        let min_tally = min_tally.into();
        self.stats
            .inc(StatType::VoteCache, DetailType::Top, Direction::In);
        if self.last_cleanup.elapsed() >= self.config.age_cutoff / 2 {
            self.cleanup();
            self.last_cleanup = Instant::now();
        }

        let mut results = Vec::new();
        for entry in self.cache.iter_by_tally() {
            if entry.tally < min_tally {
                break;
            }
            results.push(TopEntry {
                hash: entry.hash,
                tally: entry.tally,
                final_tally: entry.final_tally,
            });
        }

        // Sort by final tally then by normal tally, descending
        results.sort_by(|a, b| {
            let res = b.final_tally.cmp(&b.final_tally);
            if res == Ordering::Equal {
                b.tally.cmp(&a.tally)
            } else {
                res
            }
        });

        results
    }

    fn cleanup(&mut self) {
        self.stats
            .inc(StatType::VoteCache, DetailType::Cleanup, Direction::In);
        let to_delete: Vec<_> = self
            .cache
            .iter()
            .filter(|i| i.last_vote.elapsed() >= self.config.age_cutoff)
            .map(|i| i.hash)
            .collect();
        for hash in to_delete {
            self.cache.remove_by_hash(&hash);
        }
    }

    pub fn collect_container_info(&self, name: String) -> ContainerInfoComponent {
        ContainerInfoComponent::Composite(
            name,
            vec![ContainerInfoComponent::Leaf(ContainerInfo {
                name: "cache".to_owned(),
                count: self.size(),
                sizeof_element: size_of::<CacheEntry>(),
            })],
        )
    }
}

#[derive(PartialEq, Eq, Debug)]
pub struct TopEntry {
    pub hash: BlockHash,
    pub tally: Amount,
    pub final_tally: Amount,
}

/// Stores votes associated with a single block hash
#[derive(Clone)]
pub struct CacheEntry {
    id: usize,
    pub hash: BlockHash,
    pub voters: Vec<VoterEntry>,
    pub tally: Amount,
    pub final_tally: Amount,
    pub last_vote: Instant,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoterEntry {
    pub representative: Account,
    pub timestamp: u64,
}

impl VoterEntry {
    pub fn new(representative: Account, timestamp: u64) -> Self {
        Self {
            representative,
            timestamp,
        }
    }
}

impl CacheEntry {
    pub fn new(id: usize, hash: BlockHash) -> Self {
        CacheEntry {
            id,
            hash,
            voters: Vec::new(),
            tally: Amount::zero(),
            final_tally: Amount::zero(),
            last_vote: Instant::now(),
        }
    }

    /// Adds a vote into a list, checks for duplicates and updates timestamp if new one is greater
    /// returns true if current tally changed, false otherwise
    pub fn vote(
        &mut self,
        representative: &Account,
        timestamp: u64,
        rep_weight: Amount,
        max_voters: usize,
    ) -> bool {
        let updated = self.vote_impl(representative, timestamp, rep_weight, max_voters);
        if updated {
            self.last_vote = Instant::now();
        }
        updated
    }

    fn vote_impl(
        &mut self,
        representative: &Account,
        timestamp: u64,
        rep_weight: Amount,
        max_voters: usize,
    ) -> bool {
        if let Some(existing) = self
            .voters
            .iter_mut()
            .find(|voter| voter.representative == *representative)
        {
            // We already have a vote from this rep
            // Update timestamp if newer but tally remains unchanged as we already counted this rep weight
            // It is not essential to keep tally up to date if rep voting weight changes, elections do tally calculations independently, so in the worst case scenario only our queue ordering will be a bit off
            if timestamp > existing.timestamp {
                existing.timestamp = timestamp;
                if timestamp == Vote::FINAL_TIMESTAMP {
                    self.final_tally = self.final_tally.wrapping_add(rep_weight);
                }
                return true;
            } else {
                return false;
            }
        }
        // Vote from an unseen representative, add to list and update tally
        if self.voters.len() < max_voters {
            self.voters
                .push(VoterEntry::new(*representative, timestamp));

            // the test vote_processor.weights sometimes causes an overflow. TODO: find out why
            self.tally = self.tally.wrapping_add(rep_weight);
            if timestamp == Vote::FINAL_TIMESTAMP {
                self.final_tally = self.final_tally.wrapping_add(rep_weight);
            }
            return true;
        }
        false
    }

    pub fn size(&self) -> usize {
        self.voters.len()
    }
}

#[derive(Default)]
pub struct CacheEntryCollection {
    sequential: BTreeMap<usize, BlockHash>,
    by_hash: HashMap<BlockHash, CacheEntry>,
    by_tally: BTreeMap<Amount, Vec<BlockHash>>,
}

impl CacheEntryCollection {
    pub fn insert(&mut self, entry: CacheEntry) {
        let old = self.sequential.insert(entry.id, entry.hash);
        debug_assert!(old.is_none());
        self.insert_by_tally(entry.tally, entry.hash);
        let old = self.by_hash.insert(entry.hash, entry);
        debug_assert!(old.is_none());
    }

    pub fn modify_by_hash<F>(&mut self, hash: &BlockHash, f: F) -> bool
    where
        F: FnOnce(&mut CacheEntry),
    {
        if let Some(entry) = self.by_hash.get_mut(hash) {
            let old_tally = entry.tally;
            f(entry);
            let new_tally = entry.tally;
            if new_tally != old_tally {
                self.remove_by_tally(&old_tally, hash);
                self.insert_by_tally(new_tally, *hash);
            }
            true
        } else {
            false
        }
    }

    fn remove_by_tally(&mut self, tally: &Amount, hash: &BlockHash) {
        let hashes = self.by_tally.get_mut(tally).unwrap();
        if hashes.len() == 1 {
            self.by_tally.remove(tally);
        } else {
            hashes.retain(|x| x != hash);
        }
    }

    fn insert_by_tally(&mut self, tally: Amount, hash: BlockHash) {
        self.by_tally.entry(tally).or_default().push(hash);
    }

    pub fn iter_by_tally(&self) -> impl Iterator<Item = &CacheEntry> {
        self.by_tally
            .iter()
            .rev()
            .flat_map(|(_, hashes)| hashes.iter().map(|h| self.by_hash.get(h).unwrap()))
    }

    pub fn pop_front(&mut self) -> Option<CacheEntry> {
        match self.sequential.pop_first() {
            Some((_, front_hash)) => {
                let entry = self.by_hash.remove(&front_hash).unwrap();
                self.remove_by_tally(&entry.tally, &front_hash);
                Some(entry)
            }
            None => None,
        }
    }

    pub fn get_by_hash(&self, hash: &BlockHash) -> Option<&CacheEntry> {
        self.by_hash.get(hash)
    }

    pub fn remove_by_hash(&mut self, hash: &BlockHash) -> Option<CacheEntry> {
        match self.by_hash.remove(hash) {
            Some(entry) => {
                self.remove_by_tally(&entry.tally, hash);
                self.sequential.remove(&entry.id);
                Some(entry)
            }
            None => None,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &CacheEntry> {
        self.by_hash.values()
    }

    pub fn len(&self) -> usize {
        self.sequential.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sequential.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mock_instant::MockClock;
    use rsnano_core::KeyPair;

    fn create_vote(rep: &KeyPair, hash: &BlockHash, timestamp_offset: u64) -> Vote {
        Vote::new(
            rep.public_key(),
            &rep.private_key(),
            timestamp_offset * 1024 * 1024,
            0,
            vec![*hash],
        )
    }

    fn create_final_vote(rep: &KeyPair, hash: &BlockHash) -> Vote {
        Vote::new_final(rep, vec![*hash])
    }

    fn test_config() -> VoteCacheConfig {
        VoteCacheConfig {
            max_size: 3,
            max_voters: 80,
            age_cutoff: Duration::from_secs(5 * 60),
        }
    }

    fn create_vote_cache() -> VoteCache {
        VoteCache::new(test_config(), Arc::new(Stats::new(Default::default())))
    }

    #[test]
    fn construction() {
        let cache = create_vote_cache();
        assert_eq!(cache.size(), 0);
        assert!(cache.empty());
        let hash = BlockHash::random();
        assert!(cache.find(&hash).is_none());
    }

    #[test]
    fn insert_one_hash() {
        let mut cache = create_vote_cache();
        let rep = KeyPair::new();
        let hash = BlockHash::from(1);
        let vote = create_vote(&rep, &hash, 1);

        cache.vote(&hash, &vote, Amount::raw(7));

        assert_eq!(cache.size(), 1);
        let peek = cache.find(&hash).unwrap();
        assert_eq!(peek.hash, hash);
        assert_eq!(peek.voters.len(), 1);
        assert_eq!(
            peek.voters.first(),
            Some(&VoterEntry::new(rep.public_key(), 1024 * 1024))
        );
        assert_eq!(peek.tally, Amount::raw(7))
    }

    /*
     * Inserts multiple votes for single hash
     * Ensures all of them can be retrieved and that tally is properly accumulated
     */
    #[test]
    fn insert_one_hash_many_votes() {
        let mut cache = create_vote_cache();

        let hash = BlockHash::random();
        let rep1 = KeyPair::new();
        let rep2 = KeyPair::new();
        let rep3 = KeyPair::new();

        let vote1 = create_vote(&rep1, &hash, 1);
        let vote2 = create_vote(&rep2, &hash, 2);
        let vote3 = create_vote(&rep3, &hash, 3);

        cache.vote(&hash, &vote1, Amount::raw(7));
        cache.vote(&hash, &vote2, Amount::raw(9));
        cache.vote(&hash, &vote3, Amount::raw(11));
        // We have 3 votes but for a single hash, so just one entry in vote cache
        assert_eq!(cache.size(), 1);
        let peek = cache.find(&hash).unwrap();
        assert_eq!(peek.voters.len(), 3);
        // Tally must be the sum of rep weights
        assert_eq!(peek.tally, Amount::raw(7 + 9 + 11));
    }

    #[test]
    fn insert_many_hashes_many_votes() {
        let mut cache = create_vote_cache();

        // There will be 3 hashes to vote for
        let hash1 = BlockHash::from(1);
        let hash2 = BlockHash::from(2);
        let hash3 = BlockHash::from(3);

        // There will be 4 reps with different weights
        let rep1 = KeyPair::new();
        let rep2 = KeyPair::new();
        let rep3 = KeyPair::new();
        let rep4 = KeyPair::new();

        // Votes: rep1 > hash1, rep2 > hash2, rep3 > hash3, rep4 > hash1 (the same as rep1)
        let vote1 = create_vote(&rep1, &hash1, 1);
        let vote2 = create_vote(&rep2, &hash2, 1);
        let vote3 = create_vote(&rep3, &hash3, 1);
        let vote4 = create_vote(&rep4, &hash1, 1);

        // Insert first 3 votes in cache
        cache.vote(&hash1, &vote1, Amount::raw(7));
        cache.vote(&hash2, &vote2, Amount::raw(9));
        cache.vote(&hash3, &vote3, Amount::raw(11));

        // Ensure all of those are properly inserted
        assert_eq!(cache.size(), 3);
        assert!(cache.find(&hash1).is_some());
        assert!(cache.find(&hash2).is_some());
        assert!(cache.find(&hash3).is_some());

        let peek1 = cache.find(&hash3).unwrap();
        assert_eq!(peek1.voters.len(), 1);
        assert_eq!(peek1.tally, Amount::raw(11));
        assert_eq!(peek1.hash, hash3);

        // Now add a vote from rep4 with the highest voting weight
        cache.vote(&hash1, &vote4, Amount::raw(13));

        let pop1 = cache.find(&hash1).unwrap();
        assert_eq!(pop1.voters.len(), 2);
        assert_eq!(pop1.tally, Amount::raw(7 + 13));
        assert_eq!(pop1.hash, hash1);

        let pop2 = cache.find(&hash3).unwrap();
        assert_eq!(pop2.voters.len(), 1);
        assert_eq!(pop2.tally, Amount::raw(11));
        assert_eq!(pop2.hash, hash3);

        // And last one should be hash2 with rep2 tally weight
        let pop3 = cache.find(&hash2).unwrap();
        assert_eq!(pop3.voters.len(), 1);
        assert_eq!(pop3.tally, Amount::raw(9));
        assert_eq!(pop3.hash, hash2);
    }

    /*
     * Ensure that duplicate votes are ignored
     */
    #[test]
    fn insert_duplicate() {
        let mut cache = create_vote_cache();

        let hash = BlockHash::from(1);
        let rep = KeyPair::new();
        let vote1 = create_vote(&rep, &hash, 1);
        let vote2 = create_vote(&rep, &hash, 1);

        cache.vote(&hash, &vote1, Amount::raw(9));
        cache.vote(&hash, &vote2, Amount::raw(9));

        assert_eq!(cache.size(), 1)
    }

    /*
     * Ensure that when processing vote from a representative that is already cached, we always update to the vote with the highest timestamp
     */
    #[test]
    fn insert_newer() {
        let mut cache = create_vote_cache();

        let hash = BlockHash::from(1);
        let rep = KeyPair::new();
        let vote1 = create_vote(&rep, &hash, 1);
        cache.vote(&hash, &vote1, Amount::raw(9));

        let vote2 = Vote::new(
            rep.public_key(),
            &rep.private_key(),
            Vote::TIMESTAMP_MAX,
            Vote::DURATION_MAX,
            vec![hash],
        );
        cache.vote(&hash, &vote2, Amount::raw(9));

        let peek2 = cache.find(&hash).unwrap();
        assert_eq!(cache.size(), 1);
        assert_eq!(peek2.voters.len(), 1);
        assert_eq!(peek2.voters.first().unwrap().timestamp, u64::MAX); // final timestamp
    }

    /*
     * Ensure that when processing vote from a representative that is already cached, votes with older timestamp are ignored
     */
    #[test]
    fn insert_older() {
        let mut cache = create_vote_cache();
        let hash = BlockHash::from(1);
        let rep = KeyPair::new();
        let vote1 = create_vote(&rep, &hash, 2);
        cache.vote(&hash, &vote1, Amount::raw(9));
        let peek1 = cache.find(&hash).unwrap().clone();

        let vote2 = create_vote(&rep, &hash, 1);
        cache.vote(&hash, &vote2, Amount::raw(9));
        let peek2 = cache.find(&hash).unwrap();

        assert_eq!(cache.size(), 1);
        assert_eq!(peek2.voters.len(), 1);
        assert_eq!(
            peek2.voters.first().unwrap().timestamp,
            peek1.voters.first().unwrap().timestamp
        ); // timestamp2 == timestamp1
    }

    /*
     * Ensure that erase functionality works
     */
    #[test]
    fn erase() {
        let mut cache = create_vote_cache();
        let hash1 = BlockHash::from(1);
        let hash2 = BlockHash::from(2);
        let hash3 = BlockHash::from(3);

        let rep1 = KeyPair::new();
        let rep2 = KeyPair::new();
        let rep3 = KeyPair::new();

        let vote1 = create_vote(&rep1, &hash1, 1);
        let vote2 = create_vote(&rep2, &hash2, 1);
        let vote3 = create_vote(&rep3, &hash3, 1);

        cache.vote(&hash1, &vote1, Amount::raw(7));
        cache.vote(&hash2, &vote2, Amount::raw(9));
        cache.vote(&hash3, &vote3, Amount::raw(11));

        assert_eq!(cache.size(), 3);
        assert!(cache.find(&hash1).is_some());
        assert!(cache.find(&hash2).is_some());
        assert!(cache.find(&hash3).is_some());

        cache.erase(&hash2);

        assert_eq!(cache.size(), 2);
        assert!(cache.find(&hash1).is_some());
        assert!(cache.find(&hash2).is_none());
        assert!(cache.find(&hash3).is_some());
        cache.erase(&hash1);
        cache.erase(&hash3);

        assert!(cache.empty());
    }

    /*
     * Ensure that when cache is overfilled, we remove the oldest entries first
     */
    #[test]
    fn overfill() {
        let mut cache = create_vote_cache();

        let hash1 = BlockHash::from(1);
        let hash2 = BlockHash::from(2);
        let hash3 = BlockHash::from(3);
        let hash4 = BlockHash::from(4);

        let rep1 = KeyPair::new();
        let rep2 = KeyPair::new();
        let rep3 = KeyPair::new();
        let rep4 = KeyPair::new();

        let vote1 = create_vote(&rep1, &hash1, 1);
        cache.vote(&hash1, &vote1, Amount::raw(1));

        let vote2 = create_vote(&rep2, &hash2, 1);
        cache.vote(&hash2, &vote2, Amount::raw(2));

        let vote3 = create_vote(&rep3, &hash3, 1);
        cache.vote(&hash3, &vote3, Amount::raw(3));

        let vote4 = create_vote(&rep4, &hash4, 1);
        cache.vote(&hash4, &vote4, Amount::raw(4));

        assert_eq!(cache.size(), 3);

        // Check that oldest votes are dropped first
        assert!(cache.find(&hash4).is_some());
        assert!(cache.find(&hash3).is_some());
        assert!(cache.find(&hash2).is_some());
        assert!(cache.find(&hash1).is_none());
    }

    /*
     * Check that when a single vote cache entry is overfilled, it ignores any new votes
     */
    #[test]
    fn overfill_entry() {
        let mut cache = create_vote_cache();
        let hash = BlockHash::from(1);

        let rep1 = KeyPair::new();
        let vote1 = create_vote(&rep1, &hash, 1);
        cache.vote(&hash, &vote1, Amount::raw(9));

        let rep2 = KeyPair::new();
        let vote2 = create_vote(&rep2, &hash, 1);
        cache.vote(&hash, &vote2, Amount::raw(9));

        let rep3 = KeyPair::new();
        let vote3 = create_vote(&rep3, &hash, 1);
        cache.vote(&hash, &vote3, Amount::raw(9));

        assert_eq!(cache.size(), 1);
    }

    #[test]
    fn change_vote_to_final_vote() {
        let mut cache = create_vote_cache();
        let hash = BlockHash::from(1);

        let rep = KeyPair::new();
        let vote = create_vote(&rep, &hash, 1);
        let final_vote = create_final_vote(&rep, &hash);
        cache.vote(&hash, &vote, Amount::raw(9));
        cache.vote(&hash, &final_vote, Amount::raw(9));

        let entry = cache.find(&hash).unwrap();
        assert_eq!(entry.tally, Amount::raw(9));
        assert_eq!(entry.final_tally, Amount::raw(9));
    }

    #[test]
    fn add_final_vote() {
        let mut cache = create_vote_cache();
        let hash = BlockHash::from(1);

        let rep = KeyPair::new();
        let vote = create_final_vote(&rep, &hash);
        cache.vote(&hash, &vote, Amount::raw(9));

        let entry = cache.find(&hash).unwrap();
        assert_eq!(entry.tally, Amount::raw(9));
        assert_eq!(entry.final_tally, Amount::raw(9));
    }

    #[test]
    fn top_empty() {
        let mut cache = create_vote_cache();
        assert_eq!(cache.top(0), Vec::new());
    }

    #[test]
    fn top_one_entry() {
        let mut cache = create_vote_cache();
        let hash = BlockHash::from(1);
        add_test_vote(&mut cache, &hash, Amount::raw(1));

        assert_eq!(
            cache.top(0),
            vec![TopEntry {
                hash,
                tally: Amount::raw(1),
                final_tally: Amount::zero()
            }]
        );
    }

    #[test]
    fn top_multiple_entries_sorted_by_tally() {
        let mut cache = create_vote_cache();
        let hash1 = BlockHash::from(1);
        let hash2 = BlockHash::from(2);
        let hash3 = BlockHash::from(3);
        add_test_vote(&mut cache, &hash1, Amount::raw(1));
        add_test_vote(&mut cache, &hash2, Amount::raw(4));
        add_test_vote(&mut cache, &hash3, Amount::raw(3));
        add_test_final_vote(&mut cache, &hash2, Amount::raw(5));
        add_test_final_vote(&mut cache, &hash3, Amount::raw(5));

        let top = cache.top(0);

        assert_eq!(top.len(), 3);
        assert_eq!(top[0].hash, hash2);
        assert_eq!(top[1].hash, hash3);
        assert_eq!(top[2].hash, hash1);
    }

    #[test]
    fn top_min_tally() {
        let mut cache = create_vote_cache();
        let hash1 = BlockHash::from(1);
        let hash2 = BlockHash::from(2);
        let hash3 = BlockHash::from(3);
        add_test_vote(&mut cache, &hash1, Amount::raw(1));
        add_test_vote(&mut cache, &hash2, Amount::raw(2));
        add_test_vote(&mut cache, &hash3, Amount::raw(3));

        let top = cache.top(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].hash, hash3);
        assert_eq!(top[1].hash, hash2);
    }

    #[test]
    fn top_age_cutoff() {
        let stats = Arc::new(Stats::new(Default::default()));
        let mut cache = VoteCache::new(test_config(), Arc::clone(&stats));
        let hash = BlockHash::from(1);
        add_test_vote(&mut cache, &hash, Amount::raw(1));
        assert_eq!(
            stats.count(StatType::VoteCache, DetailType::Cleanup, Direction::In),
            0
        );
        MockClock::advance(Duration::from_secs(150));
        assert_eq!(cache.top(0).len(), 1);
        assert_eq!(
            stats.count(StatType::VoteCache, DetailType::Cleanup, Direction::In),
            1
        );
        MockClock::advance(Duration::from_secs(150));
        assert_eq!(cache.top(0).len(), 0);
        assert_eq!(
            stats.count(StatType::VoteCache, DetailType::Cleanup, Direction::In),
            2
        );
    }

    fn add_test_vote(cache: &mut VoteCache, hash: &BlockHash, rep_weight: Amount) {
        let vote = create_vote(&KeyPair::new(), &hash, 0);
        cache.vote(&hash, &vote, rep_weight);
    }

    fn add_test_final_vote(cache: &mut VoteCache, hash: &BlockHash, rep_weight: Amount) {
        let vote = create_final_vote(&KeyPair::new(), &hash);
        cache.vote(&hash, &vote, rep_weight);
    }
}
