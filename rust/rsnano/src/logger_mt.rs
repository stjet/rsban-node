pub(crate) trait Logger {
    fn try_log(&self, message: &str) -> bool;
}
