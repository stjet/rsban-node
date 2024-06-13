use rsnano_core::utils::ContainerInfoComponent;
use std::{
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
    time::Duration,
};

pub struct VoteRouter {
    thread: Mutex<Option<JoinHandle<()>>>,
    shared: Arc<(Condvar, Mutex<State>)>,
}

impl VoteRouter {
    pub fn new() -> Self {
        Self {
            thread: Mutex::new(None),
            shared: Arc::new((Condvar::new(), Mutex::new(State { stopped: false }))),
        }
    }

    pub fn start(&self) {
        let shared = self.shared.clone();
        *self.thread.lock().unwrap() = Some(
            std::thread::Builder::new()
                .name("Voute router".to_owned())
                .spawn(move || {
                    let (condition, state) = &*shared;
                    let mut guard = state.lock().unwrap();
                    while !guard.stopped {
                        guard.run_one();
                        guard = condition
                            .wait_timeout_while(guard, Duration::from_secs(15), |g| !g.stopped)
                            .unwrap()
                            .0;
                    }
                })
                .unwrap(),
        )
    }

    pub fn stop(&self) {
        self.shared.1.lock().unwrap().stopped = true;
        self.shared.0.notify_all();
        if let Some(thread) = self.thread.lock().unwrap().take() {
            thread.join().unwrap();
        }
    }

    pub fn collect_container_info(&self, name: impl Into<String>) -> ContainerInfoComponent {
        ContainerInfoComponent::Composite(name.into(), vec![])
    }
}

struct State {
    stopped: bool,
}

impl State {
    fn run_one(&self) {}
}
