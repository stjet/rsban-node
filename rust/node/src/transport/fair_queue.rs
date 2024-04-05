use rsnano_core::utils::{ContainerInfo, ContainerInfoComponent};

use super::ChannelEnum;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Weak},
    time::{Duration, Instant},
};

struct Origin<S> {
    source: S,
    channel: Option<Arc<ChannelEnum>>,
}

#[derive(Clone)]
struct OriginEntry<S>
where
    S: Ord + Copy,
{
    source: S,

    // Optional is needed to distinguish between a source with no associated channel and a source with an expired channel
    // TODO: Store channel as shared_ptr after networking fixes are done
    maybe_channel: Option<Weak<ChannelEnum>>,
}

impl<S> OriginEntry<S>
where
    S: Ord + Copy,
{
    pub fn new(source: S) -> Self {
        Self {
            source,
            maybe_channel: None,
        }
    }

    pub fn new_with_channel(source: S, channel: &Arc<ChannelEnum>) -> Self {
        Self {
            source,
            maybe_channel: Some(Arc::downgrade(channel)),
        }
    }

    pub fn is_alive(&self) -> bool {
        if let Some(maybe_channel) = &self.maybe_channel {
            if let Some(channel) = maybe_channel.upgrade() {
                channel.is_alive()
            } else {
                false
            }
        } else {
            // Some sources (eg. local RPC) don't have an associated channel, never remove their queue
            true
        }
    }
}

impl<S> From<&Origin<S>> for OriginEntry<S>
where
    S: Ord + Copy,
{
    fn from(value: &Origin<S>) -> Self {
        Self {
            source: value.source,
            maybe_channel: value.channel.as_ref().map(Arc::downgrade),
        }
    }
}

impl<S> From<&OriginEntry<S>> for Origin<S>
where
    S: Ord + Copy,
{
    fn from(value: &OriginEntry<S>) -> Self {
        Self {
            source: value.source,
            channel: value.maybe_channel.as_ref().and_then(|c| c.upgrade()),
        }
    }
}

impl<S> PartialEq for OriginEntry<S>
where
    S: Ord + Copy,
{
    fn eq(&self, other: &Self) -> bool {
        matches!(self.cmp(&other), Ordering::Equal)
    }
}

impl<S> Eq for OriginEntry<S> where S: Ord + Copy {}

impl<S> Ord for OriginEntry<S>
where
    S: Ord + Copy,
{
    fn cmp(&self, other: &Self) -> Ordering {
        let source_ordering = self.source.cmp(&other.source);
        if !matches!(source_ordering, std::cmp::Ordering::Equal) {
            return source_ordering;
        }

        match (self.maybe_channel.as_ref(), other.maybe_channel.as_ref()) {
            (None, None) => Ordering::Equal,
            (Some(c1), Some(c2)) => c1.as_ptr().cmp(&c2.as_ptr()),
            (Some(_), None) => Ordering::Greater,
            (None, Some(_)) => Ordering::Less,
        }
    }
}

impl<S> PartialOrd for OriginEntry<S>
where
    S: Ord + Copy,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

struct Entry<R> {
    requests: VecDeque<R>,
    priority: usize,
    max_size: usize,
}

impl<R> Entry<R> {
    pub fn new(max_size: usize, priority: usize) -> Self {
        Self {
            max_size,
            priority,
            requests: Default::default(),
        }
    }

    pub fn pop(&mut self) -> Option<R> {
        self.requests.pop_front()
    }

    pub fn push(&mut self, request: R) -> bool {
        if self.requests.len() < self.max_size {
            self.requests.push_back(request);
            true // Added
        } else {
            false // Dropped
        }
    }

    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    pub fn len(&self) -> usize {
        self.requests.len()
    }
}

struct FairQueue<R, S>
where
    S: Ord + Copy,
{
    queues: BTreeMap<OriginEntry<S>, Entry<R>>,
    last_update: Instant,
    current_queue_key: Option<OriginEntry<S>>,
    max_size_query: Box<dyn Fn(&Origin<S>) -> usize>,
    priority_query: Box<dyn Fn(&Origin<S>) -> usize>,
    counter: usize,
}

impl<R, S> FairQueue<R, S>
where
    S: Ord + Copy,
{
    pub fn len(&self, source: &Origin<S>) -> usize {
        self.queues
            .get(&source.into())
            .map(|q| q.len())
            .unwrap_or_default()
    }

    pub fn max_len(&self, source: &Origin<S>) -> usize {
        self.queues
            .get(&source.into())
            .map(|q| q.max_size)
            .unwrap_or_default()
    }

    pub fn priority(&self, source: &Origin<S>) -> usize {
        self.queues
            .get(&source.into())
            .map(|q| q.priority)
            .unwrap_or_default()
    }

    pub fn total_len(&self) -> usize {
        self.queues.values().map(|q| q.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.queues.values().all(|q| q.is_empty())
    }

    pub fn queues_len(&self) -> usize {
        self.queues.len()
    }

    pub fn clear(&mut self) {
        self.queues.clear();
    }

    /// Should be called periodically to clean up stale channels and update queue priorities and max sizes
    pub fn periodic_update(&mut self, interval: Duration) -> bool {
        if self.last_update.elapsed() < interval {
            return false; // Not updated
        }
        self.last_update = Instant::now();
        self.cleanup();
        self.update();
        true // Updated
    }

    /// Push a request to the appropriate queue based on the source
    /// Request will be dropped if the queue is full
    /// @return true if added, false if dropped
    pub fn push(&mut self, request: R, source: Origin<S>) -> bool {
        let origin_entry = OriginEntry::from(&source);
        let entry = self.queues.entry(origin_entry).or_insert_with(|| {
            let max_size = (self.max_size_query)(&source);
            let priority = (self.priority_query)(&source);
            Entry::new(max_size, priority)
        });
        entry.push(request)
    }

    pub fn next(&mut self) -> Option<(R, Origin<S>)> {
        let should_seek = match &self.current_queue_key {
            Some(key) => match self.queues.get(key) {
                Some(queue) => {
                    if queue.is_empty() {
                        true
                    } else {
                        // Allow up to `queue.priority` requests to be processed before moving to the next queue
                        self.counter >= queue.priority
                    }
                }
                None => true,
            },
            None => true,
        };

        if should_seek {
            self.seek_next();
        }

        let it = self.current_queue_key.as_ref()?;
        let queue = self.queues.get_mut(it).unwrap();
        self.counter += 1;
        Some((queue.pop().unwrap(), it.into()))
    }

    pub fn next_batch(&mut self, max_count: usize) -> VecDeque<(R, Origin<S>)> {
        // TODO: Naive implementation, could be optimized
        let mut result = VecDeque::new();
        loop {
            if self.is_empty() || result.len() >= max_count {
                break;
            }
            result.push_back(self.next().unwrap());
        }
        result
    }

    pub fn collect_container_info(&self, name: impl Into<String>) -> ContainerInfoComponent {
        ContainerInfoComponent::Composite(
            name.into(),
            vec![
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "queues".to_string(),
                    count: self.queues.len(),
                    sizeof_element: std::mem::size_of::<OriginEntry<S>>()
                        + std::mem::size_of::<Entry<R>>(),
                }),
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "total_size".to_string(),
                    count: self.total_len(),
                    sizeof_element: std::mem::size_of::<OriginEntry<S>>()
                        + std::mem::size_of::<Entry<R>>(),
                }),
            ],
        )
    }

    fn seek_next(&mut self) {
        self.counter = 0;
        //TODO unwraps and inefficient access! Endless loop?
        loop {
            if let Some(current) = self.current_queue_key.take() {
                let mut it = self.queues.range(current..);
                if let Some(_) = it.next() {
                    self.current_queue_key = it.next().map(|(k, _)| k.clone());
                }
            }

            if self.current_queue_key.is_none() {
                self.current_queue_key = Some(self.queues.first_key_value().unwrap().0.clone());
            }

            if !self
                .queues
                .get(self.current_queue_key.as_ref().unwrap())
                .unwrap()
                .is_empty()
            {
                break;
            }
        }
    }

    fn cleanup(&mut self) {
        self.current_queue_key = None;
        self.queues.retain(|k, _| k.is_alive());
    }

    fn update(&mut self) {
        for (source, queue) in self.queues.iter_mut() {
            queue.max_size = (self.max_size_query)(&source.into());
            queue.priority = (self.priority_query)(&source.into());
        }
    }
}
