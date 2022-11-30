use std::any::Any;

pub trait PropertyTreeReader {
    fn get_string(&self, path: &str) -> anyhow::Result<String>;
}

pub trait PropertyTreeWriter {
    fn clear(&mut self) -> anyhow::Result<()>;
    fn put_string(&mut self, path: &str, value: &str) -> anyhow::Result<()>;
    fn put_u64(&mut self, path: &str, value: u64) -> anyhow::Result<()>;
    fn new_writer(&self) -> Box<dyn PropertyTreeWriter>;
    fn push_back(&mut self, path: &str, value: &dyn PropertyTreeWriter);
    fn add_child(&mut self, path: &str, value: &dyn PropertyTreeWriter);
    fn put_child(&mut self, path: &str, value: &dyn PropertyTreeWriter);
    fn add(&mut self, path: &str, value: &str) -> anyhow::Result<()>;
    fn as_any(&self) -> &dyn Any;
    fn to_json(&self) -> String;
}
