use std::sync::atomic::{AtomicU32, Ordering};

pub struct LmdbWalletStore {
    db_handle: AtomicU32,
}

impl LmdbWalletStore {
    pub fn new() -> Self {
        Self {
            db_handle: AtomicU32::new(0),
        }
    }

    pub fn db_handle(&self) -> u32 {
        self.db_handle.load(Ordering::SeqCst)
    }

    pub fn set_db_handle(&self, handle: u32) {
        self.db_handle.store(handle, Ordering::SeqCst);
    }
}
