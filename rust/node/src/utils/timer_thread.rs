use std::{
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
    time::Duration,
};

// Runs a task periodically in it's own thread
pub struct TimerThread<T: Runnable + 'static> {
    thread_name: String,
    task: Mutex<Option<T>>,
    thread: Mutex<Option<JoinHandle<()>>>,
    interval: Duration,
    stop: Arc<(Mutex<bool>, Condvar)>,
}

impl<T: Runnable> TimerThread<T> {
    pub fn new(name: impl Into<String>, task: T, interval: Duration) -> Self {
        Self {
            thread_name: name.into(),
            task: Mutex::new(Some(task)),
            thread: Mutex::new(None),
            interval,
            stop: Arc::new((Mutex::new(false), Condvar::new())),
        }
    }

    pub fn start(&self) {
        let mut task = self
            .task
            .lock()
            .unwrap()
            .take()
            .expect("task already taken");

        let stop = Arc::clone(&self.stop);
        let interval = self.interval.clone();
        let handle = std::thread::Builder::new()
            .name(self.thread_name.clone())
            .spawn(move || {
                let mut stopped_guard = stop.0.lock().unwrap();
                while !*stopped_guard {
                    stopped_guard = stop
                        .1
                        .wait_timeout_while(stopped_guard, interval, |stopped| !*stopped)
                        .unwrap()
                        .0;
                    drop(stopped_guard);

                    task.run();

                    stopped_guard = stop.0.lock().unwrap();
                }
            })
            .unwrap();

        *self.thread.lock().unwrap() = Some(handle);
    }

    pub fn stop(&self) {
        *self.stop.0.lock().unwrap() = true;
        self.stop.1.notify_all();
        if let Some(handle) = self.thread.lock().unwrap().take() {
            handle.join().unwrap();
        }
    }
}

impl<T: Runnable> Drop for TimerThread<T> {
    fn drop(&mut self) {
        self.stop();
    }
}

pub trait Runnable: Send {
    fn run(&mut self);
}
