use crate::{Root, WorkVersion};

use super::{WorkPool, WorkThresholds};

/// The StubWorkPool assumes work == difficulty
pub struct StubWorkPool {
    thresholds: WorkThresholds,
}

impl StubWorkPool {
    pub fn new(thresholds: WorkThresholds) -> Self {
        Self { thresholds }
    }
}

impl WorkPool for StubWorkPool {
    fn generate_async(
        &self,
        _version: WorkVersion,
        _root: Root,
        difficulty: u64,
        done: Option<Box<dyn Fn(Option<u64>) + Send>>,
    ) {
        if let Some(done) = done {
            done(Some(difficulty))
        }
    }

    fn generate_dev(&self, _root: Root, difficulty: u64) -> Option<u64> {
        Some(difficulty)
    }

    fn generate_dev2(&self, _root: Root) -> Option<u64> {
        Some(self.thresholds.base)
    }

    fn generate(&self, _version: WorkVersion, _root: Root, difficulty: u64) -> Option<u64> {
        Some(difficulty)
    }
}
