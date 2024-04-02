mod confirming_set;

pub use confirming_set::*;
use rsnano_core::{BlockEnum, BlockHash};
use std::sync::Arc;

type BlockCallback = Box<dyn FnMut(&Arc<BlockEnum>) + Send>;
type BlockHashCallback = Box<dyn FnMut(BlockHash) + Send>;
