use std::sync::Arc;

pub trait BootstrapClientObserver {
    fn bootstrap_client_closed(&self);
    fn to_weak(&self) -> Box<dyn BootstrapClientObserverWeakPtr>;
}

pub trait BootstrapClientObserverWeakPtr {
    fn upgrade(&self) -> Option<Arc<dyn BootstrapClientObserver>>;
}

pub struct BootstrapClient {
    observer: Box<dyn BootstrapClientObserverWeakPtr>,
}

impl BootstrapClient {
    pub fn new(observer: Arc<dyn BootstrapClientObserver>) -> Self {
        Self {
            observer: observer.to_weak(),
        }
    }
}

impl Drop for BootstrapClient {
    fn drop(&mut self) {
        if let Some(observer) = self.observer.upgrade() {
            observer.bootstrap_client_closed();
        }
    }
}
