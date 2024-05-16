use super::{CpuWorkGenerator, WorkGenerator, WorkTicket};
use crate::{Root, WorkVersion};
use std::time::Duration;

pub type OpenClWorkFunc = dyn Fn(WorkVersion, Root, u64, &WorkTicket) -> Option<u64> + Send + Sync;

pub(crate) struct OpenClWorkGenerator {
    opencl: Box<OpenClWorkFunc>,
    cpu_gen: CpuWorkGenerator,
}

// Tries to create PoW with OpenCL. If no OpenCL callback is given, then PoW will be generated on the CPUs
impl OpenClWorkGenerator {
    pub fn new(rate_limiter: Duration, opencl: Box<OpenClWorkFunc>) -> Self {
        Self {
            opencl,
            cpu_gen: CpuWorkGenerator::new(rate_limiter),
        }
    }
}

impl WorkGenerator for OpenClWorkGenerator {
    fn create(
        &mut self,
        version: WorkVersion,
        item: &Root,
        min_difficulty: u64,
        work_ticket: &WorkTicket,
    ) -> Option<u64> {
        let work = (self.opencl)(version, *item, min_difficulty, work_ticket);

        if work.is_some() {
            work
        } else {
            self.cpu_gen
                .create(version, item, min_difficulty, work_ticket)
        }
    }
}
