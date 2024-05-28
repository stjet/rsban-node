use std::{
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
    time::Duration,
};

use rsnano_core::utils::{OutputListenerMt, OutputTrackerMt};

// Runs a task periodically in it's own thread
pub struct TimerThread<T: Runnable + 'static> {
    thread_name: String,
    task: Mutex<Option<T>>,
    thread: Mutex<Option<JoinHandle<()>>>,
    interval: Duration,
    cancel_token: CancellationToken,
    run_immediately: bool,
    start_listener: OutputListenerMt<TimerStartEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimerStartEvent {
    pub thread_name: String,
    pub interval: Duration,
    pub run_immediately: bool,
}

impl<T: Runnable> TimerThread<T> {
    pub fn new(name: impl Into<String>, task: T, interval: Duration) -> Self {
        Self {
            thread_name: name.into(),
            task: Mutex::new(Some(task)),
            thread: Mutex::new(None),
            interval,
            cancel_token: CancellationToken::new(),
            run_immediately: false,
            start_listener: OutputListenerMt::new(),
        }
    }

    pub fn new_run_immedately(name: impl Into<String>, task: T, interval: Duration) -> Self {
        Self {
            thread_name: name.into(),
            task: Mutex::new(Some(task)),
            thread: Mutex::new(None),
            interval,
            cancel_token: CancellationToken::new(),
            run_immediately: true,
            start_listener: OutputListenerMt::new(),
        }
    }

    pub fn is_running(&self) -> bool {
        self.thread.lock().unwrap().is_some()
    }

    pub fn track_start(&self) -> Arc<OutputTrackerMt<TimerStartEvent>> {
        self.start_listener.track()
    }

    pub fn start(&self) {
        self.start_listener.emit(TimerStartEvent {
            thread_name: self.thread_name.clone(),
            interval: self.interval,
            run_immediately: self.run_immediately,
        });

        let mut task = self
            .task
            .lock()
            .unwrap()
            .take()
            .expect("task already taken");

        let cancel_token = self.cancel_token.clone();
        let interval = self.interval.clone();
        let run_immediately = self.run_immediately;
        let handle = std::thread::Builder::new()
            .name(self.thread_name.clone())
            .spawn(move || {
                if run_immediately {
                    task.run();
                }

                while !cancel_token.wait_for_cancellation(interval) {
                    task.run();
                }
            })
            .unwrap();

        *self.thread.lock().unwrap() = Some(handle);
    }

    pub fn stop(&self) {
        self.cancel_token.cancel();
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

#[derive(Clone)]
pub struct CancellationToken(Arc<CancellationTokenData>);

struct CancellationTokenData {
    stopped: Mutex<bool>,
    condition: Condvar,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self(Arc::new(CancellationTokenData {
            stopped: Mutex::new(false),
            condition: Condvar::new(),
        }))
    }

    pub fn wait_for_cancellation(&self, timeout: Duration) -> bool {
        let mut stopped = self.0.stopped.lock().unwrap();
        if *stopped {
            return true;
        }
        stopped = self
            .0
            .condition
            .wait_timeout_while(stopped, timeout, |stop| !*stop)
            .unwrap()
            .0;
        *stopped
    }

    pub fn cancel(&self) {
        *self.0.stopped.lock().unwrap() = true;
        self.0.condition.notify_all();
    }
}
