use std::sync::Arc;

use super::{is_tokio_enabled, IoContext};

pub struct AsyncRuntime {
    pub cpp: Arc<dyn IoContext>,
    pub tokio: tokio::runtime::Runtime,
}

impl AsyncRuntime {
    pub fn new(cpp: Arc<dyn IoContext>, tokio: tokio::runtime::Runtime) -> Self {
        Self { cpp, tokio }
    }

    pub fn post<F>(&self, action: F)
    where
        F: FnOnce() + Send + 'static,
    {
        if is_tokio_enabled() {
            self.tokio.spawn_blocking(action);
        } else {
            self.cpp.post(Box::new(move || action()));
        }
    }
}
