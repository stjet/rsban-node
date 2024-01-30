pub trait Logger: Send + Sync {
    fn try_log(&self, message: &str) -> bool;
    fn always_log(&self, message: &str);
}
pub struct NullLogger {}

impl NullLogger {
    pub fn new() -> Self {
        Self {}
    }
}

impl Logger for NullLogger {
    fn try_log(&self, _message: &str) -> bool {
        false
    }

    fn always_log(&self, _message: &str) {}
}

pub struct ConsoleLogger {}

impl ConsoleLogger {
    pub fn new() -> Self {
        Self {}
    }
}

impl Logger for ConsoleLogger {
    fn try_log(&self, message: &str) -> bool {
        println!("{}", message);
        true
    }

    fn always_log(&self, message: &str) {
        println!("{}", message);
    }
}
