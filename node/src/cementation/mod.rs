mod confirming_set;

pub use confirming_set::*;
use rsnano_core::Block;
use std::sync::Arc;

type BlockCallback = Box<dyn FnMut(&Arc<Block>) + Send>;
type BatchCementedCallback = Box<dyn FnMut(&CementedNotification) + Send>;
