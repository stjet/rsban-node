use std::sync::{Arc, Mutex};

pub struct ConfirmationHeightProcessor {
    pub guarded_data: Arc<Mutex<GuardedData>>,
}

impl ConfirmationHeightProcessor {
    pub fn new() -> Self {
        Self {
            guarded_data: Arc::new(Mutex::new(GuardedData {})),
        }
    }
}

pub struct GuardedData {}
