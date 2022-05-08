pub(crate) trait Logger: Send + Sync {
    fn try_log(&self, message: &str) -> bool;
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
}
