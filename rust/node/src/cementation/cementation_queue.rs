use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use rsnano_core::{
    utils::{ContainerInfo, ContainerInfoComponent},
    BlockChainSection,
};

pub(crate) struct CementationQueue {
    queue: VecDeque<BlockChainSection>,
    queue_len: Arc<AtomicUsize>,
}

impl CementationQueue {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            queue_len: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn push_back(&mut self, details: BlockChainSection) {
        self.queue.push_back(details);
        self.queue_len.fetch_add(1, Ordering::Relaxed);
    }

    pub fn front(&self) -> Option<&BlockChainSection> {
        self.queue.front()
    }

    pub fn pop_front(&mut self) -> Option<BlockChainSection> {
        let item = self.queue.pop_front();
        if item.is_some() {
            self.queue_len.fetch_sub(1, Ordering::Relaxed);
        }
        item
    }

    pub fn total_pending_blocks(&self) -> usize {
        self.queue
            .iter()
            .map(|i| (i.top_height - i.bottom_height + 1) as usize)
            .sum()
    }

    pub fn container_info(&self) -> CementationQueueContainerInfo {
        CementationQueueContainerInfo {
            queue_len: self.queue_len.clone(),
        }
    }
}

pub(crate) struct CementationQueueContainerInfo {
    queue_len: Arc<AtomicUsize>,
}

impl CementationQueueContainerInfo {
    pub fn collect(&self, name: String) -> ContainerInfoComponent {
        ContainerInfoComponent::Leaf(ContainerInfo {
            name,
            count: self.queue_len.load(Ordering::Relaxed),
            sizeof_element: std::mem::size_of::<BlockChainSection>(),
        })
    }
}
