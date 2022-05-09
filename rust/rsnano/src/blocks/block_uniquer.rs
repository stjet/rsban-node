use std::{
    collections::HashMap,
    rc::Weak,
    sync::{Arc, Mutex, RwLock},
};

use crate::{BlockEnum, BlockHash};

pub(crate) struct BlockUniquer {
    blocks: Mutex<HashMap<BlockHash, Weak<RwLock<BlockEnum>>>>,
}

impl BlockUniquer {
    pub(crate) fn new() -> Self {
        Self {
            blocks: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn unique(&self, block: &Arc<RwLock<BlockEnum>>) -> Arc<RwLock<BlockEnum>> {
        todo!()
    }

    pub(crate) fn size(&self) -> usize {
        self.blocks.lock().unwrap().len()
    }
}
