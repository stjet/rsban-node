use super::ChannelId;
use rsnano_messages::Keepalive;
use std::collections::HashMap;

/// Keeps the last keepalive message per channel in memory, so that we can
/// later use that information, when we want to connect to more nodes
pub(crate) struct KeepaliveCache {
    entries: HashMap<ChannelId, Keepalive>,
}

impl Default for KeepaliveCache {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
}

impl KeepaliveCache {
    pub fn insert(&mut self, channel_id: ChannelId, keepalive: Keepalive) {
        self.entries.insert(channel_id, keepalive);
    }

    //TODO: randomize
    pub fn pop(&mut self) -> Option<Keepalive> {
        if let Some(&channel_id) = self.entries.keys().next() {
            self.entries.remove(&channel_id)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_core::utils::TEST_ENDPOINT_3;

    #[test]
    fn empty() {
        let cache = KeepaliveCache::default();
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn insert_one() {
        let mut cache = KeepaliveCache::default();
        cache.insert(ChannelId::from(1), Keepalive::new_test_instance());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn pop() {
        let mut cache = KeepaliveCache::default();
        let keepalive = Keepalive::new_test_instance();
        cache.insert(ChannelId::from(1), keepalive.clone());

        let popped = cache.pop().unwrap();

        assert_eq!(popped, keepalive);
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn insert_two_for_different_channel() {
        let mut cache = KeepaliveCache::default();
        cache.insert(ChannelId::from(1), Keepalive::new_test_instance());
        cache.insert(ChannelId::from(2), Keepalive::new_test_instance());
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn inserting_for_same_channel_should_replace_previous_entry() {
        let mut cache = KeepaliveCache::default();
        cache.insert(ChannelId::from(1), Keepalive::new_test_instance());
        let newest_keepalive = Keepalive {
            peers: [TEST_ENDPOINT_3; 8],
        };
        cache.insert(ChannelId::from(1), newest_keepalive.clone());
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.pop(), Some(newest_keepalive))
    }
}
