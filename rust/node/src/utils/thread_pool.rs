use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use timer::Timer;

pub trait ThreadPool: Send + Sync {
    fn push_task(&self, callback: Box<dyn FnMut() + Send>);
    fn add_delayed_task(&self, delay: Duration, callback: Box<dyn FnMut() + Send>);
}

pub struct ThreadPoolImpl {
    data: Arc<Mutex<Option<ThreadPoolData>>>,
    stopped: Arc<Mutex<bool>>,
}

struct ThreadPoolData {
    pool: threadpool::ThreadPool,
    timer: Timer,
}

impl ThreadPoolData {
    fn push_task(&self, mut callback: Box<dyn FnMut() + Send>) {
        self.pool.execute(move || callback());
    }
}

impl ThreadPoolImpl {
    pub fn new(num_threads: usize, thread_name: String) -> Self {
        Self {
            stopped: Arc::new(Mutex::new(false)),
            data: Arc::new(Mutex::new(Some(ThreadPoolData {
                pool: threadpool::Builder::new()
                    .num_threads(num_threads)
                    .thread_name(thread_name)
                    .build(),
                timer: Timer::new(),
            }))),
        }
    }

    pub fn stop(&self) {
        let mut stopped_guard = self.stopped.lock().unwrap();
        if !*stopped_guard {
            let mut data_guard = self.data.lock().unwrap();
            *stopped_guard = true;
            drop(stopped_guard);
            if let Some(data) = data_guard.take() {
                data.pool.join();
            }
        }
    }
}

impl ThreadPool for ThreadPoolImpl {
    fn push_task(&self, callback: Box<dyn FnMut() + Send>) {
        let stopped_guard = self.stopped.lock().unwrap();
        if !*stopped_guard {
            let data_guard = self.data.lock().unwrap();
            drop(stopped_guard);
            if let Some(data) = data_guard.as_ref() {
                data.push_task(callback);
            }
        }
    }

    fn add_delayed_task(&self, delay: Duration, callback: Box<dyn FnMut() + Send>) {
        let stopped_guard = self.stopped.lock().unwrap();
        if !*stopped_guard {
            let data_guard = self.data.lock().unwrap();
            drop(stopped_guard);
            let mut option_callback = Some(callback);
            let data_clone = self.data.clone();
            let stopped_clone = self.stopped.clone();
            if let Some(data) = data_guard.as_ref() {
                data.timer
                    .schedule_with_delay(chrono::Duration::from_std(delay).unwrap(), move || {
                        if let Some(cb) = option_callback.take() {
                            let stopped_guard = stopped_clone.lock().unwrap();
                            if !*stopped_guard {
                                let data_guard = data_clone.lock().unwrap();
                                drop(stopped_guard);
                                if let Some(data) = data_guard.as_ref() {
                                    data.push_task(cb);
                                }
                            }
                        }
                    })
                    .ignore();
            }
        }
    }
}

//todo collect_container_info

impl Drop for ThreadPoolImpl {
    fn drop(&mut self) {
        self.stop()
    }
}
