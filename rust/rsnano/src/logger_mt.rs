pub(crate) trait Logger: Send + Sync {
    fn try_log(&self, message: &str) -> bool;
}
