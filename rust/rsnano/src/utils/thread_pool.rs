use std::time::Duration;

pub trait ThreadPool {
    fn add_timed_task(&self, delay: Duration, callback: Box<dyn Fn()>);
}
