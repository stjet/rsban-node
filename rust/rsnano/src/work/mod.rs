mod work_thresholds;
pub use work_thresholds::WorkThresholds;

mod work_pool;
pub(crate) use work_pool::WorkGenerator;
pub use work_pool::WorkPool;
#[cfg(test)]
pub(crate) use work_pool::DEV_WORK_POOL;

mod xorshift;
pub(crate) use xorshift::XorShift1024Star;

mod work_queue;
pub(crate) use work_queue::{WorkItem, WorkQueueCoordinator, WorkTicket};

mod cpu_work_generator;
pub(crate) use cpu_work_generator::CpuWorkGenerator;

mod opencl_work_generator;
pub(crate) use opencl_work_generator::{OpenClWorkFunc, OpenClWorkGenerator};

mod work_thread;
pub(crate) use work_thread::WorkThread;

pub(crate) trait WorkRng {
    fn next_work(&mut self) -> u64;
}
