use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use rsnano_core::{
    utils::{ContainerInfo, ContainerInfoComponent},
    Account, BlockHash,
};

#[derive(Clone, Default)]
pub(crate) struct WriteDetails {
    pub account: Account,
    pub bottom_height: u64,
    // This is the first block hash (bottom most) which is not cemented
    pub bottom_hash: BlockHash,
    // Desired cemented frontier
    pub top_height: u64,
    pub top_hash: BlockHash,
}

pub(crate) struct WriteDetailsQueue {
    queue: VecDeque<WriteDetails>,
    queue_len: Arc<AtomicUsize>,
}

impl WriteDetailsQueue {
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

    pub fn push_back(&mut self, details: WriteDetails) {
        self.queue.push_back(details);
        self.queue_len.fetch_add(1, Ordering::Relaxed);
    }

    pub fn front(&self) -> Option<&WriteDetails> {
        self.queue.front()
    }

    pub fn pop_front(&mut self) -> Option<WriteDetails> {
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

    pub fn container_info(&self) -> WriteDetailsContainerInfo {
        WriteDetailsContainerInfo {
            queue_len: self.queue_len.clone(),
        }
    }
}

pub(crate) struct WriteDetailsContainerInfo {
    queue_len: Arc<AtomicUsize>,
}

impl WriteDetailsContainerInfo {
    pub fn collect(&self, name: String) -> ContainerInfoComponent {
        ContainerInfoComponent::Leaf(ContainerInfo {
            name,
            count: self.queue_len.load(Ordering::Relaxed),
            sizeof_element: std::mem::size_of::<WriteDetails>(),
        })
    }
}
