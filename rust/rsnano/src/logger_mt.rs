pub(crate) trait Logger: Send + Sync {
    fn try_log(&self, message: &str) -> bool;
    fn always_log(&self, message: &str);
}

pub(crate) struct NullLogger {}

impl NullLogger {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl Logger for NullLogger {
    fn try_log(&self, _message: &str) -> bool {
        false
    }

    fn always_log(&self, _message: &str) {}
}
