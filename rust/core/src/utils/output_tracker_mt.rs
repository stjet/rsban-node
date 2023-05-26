use std::sync::{Arc, Mutex, Weak};

// Multi threaded output tracker
pub struct OutputTrackerMt<T: Clone + 'static> {
    output: Mutex<Vec<T>>,
}

impl<T: Clone + 'static> OutputTrackerMt<T> {
    pub fn new() -> Self {
        Self {
            output: Mutex::new(Vec::new()),
        }
    }

    pub fn add(&self, t: T) {
        self.output.lock().unwrap().push(t);
    }

    pub fn output(&self) -> Vec<T> {
        self.output.lock().unwrap().clone()
    }
}

pub struct OutputListenerMt<T: Clone + 'static> {
    trackers: Mutex<Vec<Weak<OutputTrackerMt<T>>>>,
}

impl<T: Clone + 'static> OutputListenerMt<T> {
    pub fn new() -> Self {
        Self {
            trackers: Mutex::new(Vec::new()),
        }
    }

    pub fn track(&self) -> Arc<OutputTrackerMt<T>> {
        let tracker = Arc::new(OutputTrackerMt::new());
        self.trackers.lock().unwrap().push(Arc::downgrade(&tracker));
        tracker
    }

    pub fn emit(&self, t: T) {
        let mut guard = self.trackers.lock().unwrap();
        let mut should_clean = false;
        for tracker in guard.iter() {
            if let Some(tracker) = tracker.upgrade() {
                tracker.add(t.clone());
            } else {
                should_clean = true;
            }
        }

        if should_clean {
            guard.retain(|t| t.strong_count() > 0);
        }
    }

    pub fn tracker_count(&self) -> usize {
        self.trackers.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_active_trackers() {
        let listener = OutputListenerMt::new();
        listener.emit("foo");
    }

    #[test]
    fn track_one_output() {
        let listener = OutputListenerMt::new();
        let tracker = listener.track();
        listener.emit("foo");
        assert_eq!(tracker.output(), vec!["foo"]);
    }

    #[test]
    fn track_multiple_outputs() {
        let listener = OutputListenerMt::new();
        let tracker = listener.track();
        listener.emit("foo");
        listener.emit("bar");
        listener.emit("test");
        assert_eq!(tracker.output(), vec!["foo", "bar", "test"]);
    }

    #[test]
    fn multiple_trackers() {
        let listener = OutputListenerMt::new();
        let tracker1 = listener.track();
        listener.emit("foo");
        let tracker2 = listener.track();
        listener.emit("bar");
        listener.emit("test");
        assert_eq!(tracker1.output(), vec!["foo", "bar", "test"]);
        assert_eq!(tracker2.output(), vec!["bar", "test"]);
    }

    #[test]
    fn stop_tracking_if_tracker_dropped() {
        let listener = OutputListenerMt::new();
        let tracker = listener.track();
        listener.emit("foo");
        assert_eq!(listener.tracker_count(), 1);
        drop(tracker);
        listener.emit("bar");
        assert_eq!(listener.tracker_count(), 0);
    }
}
