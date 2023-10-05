use super::ElectionStatus;
use std::sync::Mutex;

pub struct Election {
    pub mutex: Mutex<ElectionStatus>,
}

impl Election {
    pub fn new() -> Self {
        Self {
            mutex: Mutex::new(ElectionStatus::default()),
        }
    }
}
