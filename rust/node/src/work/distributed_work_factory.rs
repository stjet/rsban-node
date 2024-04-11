use crate::config::{NodeConfig, Peer};
use rsnano_core::{work::WorkPoolImpl, Account, BlockEnum, Root, WorkVersion};
use std::{ffi::c_void, sync::Arc};

pub struct DistributedWorkFactory {
    /// Pointer to the C++ implementation
    factory_pointer: *mut c_void,
    config: NodeConfig,
    work_pool: Arc<WorkPoolImpl>,
}

impl DistributedWorkFactory {
    pub fn new(
        factory_pointer: *mut c_void,
        config: NodeConfig,
        work_pool: Arc<WorkPoolImpl>,
    ) -> Self {
        Self {
            factory_pointer,
            config,
            work_pool,
        }
    }

    pub fn make_blocking_block(&self, block: &mut BlockEnum, difficulty: u64) -> Option<u64> {
        unsafe {
            MAKE_BLOCKING.expect("MAKE_BLOCKING missing")(self.factory_pointer, block, difficulty)
        }
    }

    pub fn make_blocking(
        &self,
        version: WorkVersion,
        root: Root,
        difficulty: u64,
        account: Option<Account>,
    ) -> Option<u64> {
        unsafe {
            MAKE_BLOCKING_2.expect("MAKE_BLOCKING_2 missing")(
                self.factory_pointer,
                version,
                root,
                difficulty,
                account,
            )
        }
    }

    pub fn work_generation_enabled(&self) -> bool {
        self.work_generation_enabled_peers(&self.config.work_peers)
    }

    pub fn work_generation_enabled_secondary(&self) -> bool {
        self.work_generation_enabled_peers(&self.config.secondary_work_peers)
    }

    pub fn work_generation_enabled_peers(&self, peers: &[Peer]) -> bool {
        !peers.is_empty() || self.work_pool.work_generation_enabled()
    }
}

pub static mut MAKE_BLOCKING: Option<fn(*mut c_void, &mut BlockEnum, u64) -> Option<u64>> = None;
pub static mut MAKE_BLOCKING_2: Option<
    fn(*mut c_void, WorkVersion, Root, u64, Option<Account>) -> Option<u64>,
> = None;

unsafe impl Send for DistributedWorkFactory {}
unsafe impl Sync for DistributedWorkFactory {}
