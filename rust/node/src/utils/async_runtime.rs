pub struct AsyncRuntime {
    pub tokio: tokio::runtime::Runtime,
}

impl AsyncRuntime {
    pub fn new(tokio: tokio::runtime::Runtime) -> Self {
        Self { tokio }
    }

    pub fn post<F>(&self, action: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.tokio.spawn_blocking(action);
    }
}
