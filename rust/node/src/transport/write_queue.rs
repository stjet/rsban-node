use super::TrafficType;
use crate::utils::ErrorCode;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
use tokio::sync::mpsc::{self};

pub(crate) struct WriteQueue {
    max_size: usize,
    queues: Mutex<Queues>,
    generic_queue: mpsc::Sender<Entry>,
    bootstrap_queue: mpsc::Sender<Entry>,
}

impl WriteQueue {
    pub fn new(max_size: usize) -> (Self, WriteQueueReceiver) {
        let (generic_tx, generic_rx) = mpsc::channel(max_size * 2);
        let (bootstrap_tx, bootstrap_rx) = mpsc::channel(max_size * 2);
        let receiver = WriteQueueReceiver::new(generic_rx, bootstrap_rx);
        (
            Self {
                max_size,
                queues: Mutex::new(Queues {
                    generic_queue: VecDeque::new(),
                    bootstrap_queue: VecDeque::new(),
                }),
                generic_queue: generic_tx,
                bootstrap_queue: bootstrap_tx,
            },
            receiver,
        )
    }

    pub fn insert(
        &self,
        buffer: Arc<Vec<u8>>,
        callback: Option<WriteCallback>,
        traffic_type: TrafficType,
    ) -> (bool, Option<WriteCallback>) {
        let mut queues = self.queues.lock().unwrap();
        let queue = queues.get_mut(traffic_type);
        if queue.len() < 2 * self.max_size {
            queue.push_back(Entry { buffer, callback });
            (true, None) // Queued
        } else {
            (false, callback) // Not queued
        }
    }

    pub fn pop(&self) -> Option<Entry> {
        let mut queues = self.queues.lock().unwrap();

        // TODO: This is a very basic prioritization, implement something more advanced and configurable
        let item = queues.generic_queue.pop_front();
        if item.is_some() {
            item
        } else {
            queues.bootstrap_queue.pop_front()
        }
    }

    pub fn clear(&self) {
        let mut queues = self.queues.lock().unwrap();
        queues.generic_queue.clear();
        queues.bootstrap_queue.clear();
    }

    pub fn capacity(&self, traffic_type: TrafficType) -> usize {
        let queues = self.queues.lock().unwrap();
        self.max_size * 2 - queues.get(traffic_type).len()
    }
}

pub(crate) struct WriteQueueReceiver {
    generic: mpsc::Receiver<Entry>,
    bootstrap: mpsc::Receiver<Entry>,
}

impl WriteQueueReceiver {
    fn new(generic: mpsc::Receiver<Entry>, bootstrap: mpsc::Receiver<Entry>) -> Self {
        Self { generic, bootstrap }
    }

    pub(crate) fn try_pop(&mut self) -> Result<Entry, mpsc::error::TryRecvError> {
        let mut result = self.generic.try_recv();
        if matches!(result, Err(mpsc::error::TryRecvError::Empty)) {
            result = self.bootstrap.try_recv();
        }
        result
    }
}

pub type WriteCallback = Box<dyn FnOnce(ErrorCode, usize) + Send>;

pub(crate) struct Entry {
    pub buffer: Arc<Vec<u8>>,
    pub callback: Option<WriteCallback>,
}

struct Queues {
    generic_queue: VecDeque<Entry>,
    bootstrap_queue: VecDeque<Entry>,
}

impl Queues {
    fn get(&self, traffic_type: TrafficType) -> &VecDeque<Entry> {
        match traffic_type {
            TrafficType::Generic => &self.generic_queue,
            TrafficType::Bootstrap => &self.bootstrap_queue,
        }
    }

    fn get_mut(&mut self, traffic_type: TrafficType) -> &mut VecDeque<Entry> {
        match traffic_type {
            TrafficType::Generic => &mut self.generic_queue,
            TrafficType::Bootstrap => &mut self.bootstrap_queue,
        }
    }
}
