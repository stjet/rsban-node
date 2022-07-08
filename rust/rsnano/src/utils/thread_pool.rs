use std::{ffi::c_void, time::Duration};

pub trait ThreadPool {
    fn add_timed_task(&self, delay: Duration, callback: Box<dyn FnOnce()>);
    fn handle(&self) -> *mut c_void;
}
