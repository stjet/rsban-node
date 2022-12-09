mod work_thresholds;
pub use work_thresholds::{WorkThresholds, WORK_THRESHOLDS_STUB};

mod work_pool;
pub(crate) use work_pool::WorkGenerator;

pub use work_pool::{WorkPool, WorkPoolImpl, STUB_WORK_POOL};

mod xorshift;
pub(crate) use xorshift::XorShift1024Star;

mod work_queue;
pub use work_queue::WorkTicket;
pub(crate) use work_queue::{WorkItem, WorkQueueCoordinator};

mod cpu_work_generator;
pub(crate) use cpu_work_generator::CpuWorkGenerator;

mod opencl_work_generator;
pub(crate) use opencl_work_generator::{OpenClWorkFunc, OpenClWorkGenerator};

mod work_thread;
pub(crate) use work_thread::WorkThread;

mod stub_work_pool;
pub use stub_work_pool::StubWorkPool;

pub(crate) trait WorkRng {
    fn next_work(&mut self) -> u64;
}
