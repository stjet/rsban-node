use bounded_vec_deque::BoundedVecDeque;
use rsnano_core::BlockHash;

pub struct ConfirmationHeightBounded {}

impl ConfirmationHeightBounded {
    pub fn new() -> Self {
        Self {}
    }
}

pub fn truncate_after(buffer: &mut BoundedVecDeque<BlockHash>, hash: &BlockHash) {
    if let Some((index, _)) = buffer.iter().enumerate().find(|(_, h)| *h != hash) {
        buffer.truncate(index);
    }
}
