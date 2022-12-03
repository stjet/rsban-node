pub trait TomlWriter {
    fn put_u16(&mut self, key: &str, value: u16, documentation: &str) -> anyhow::Result<()>;
    fn put_u32(&mut self, key: &str, value: u32, documentation: &str) -> anyhow::Result<()>;
    fn put_u64(&mut self, key: &str, value: u64, documentation: &str) -> anyhow::Result<()>;
    fn put_i64(&mut self, key: &str, value: i64, documentation: &str) -> anyhow::Result<()>;
    fn put_str(&mut self, key: &str, value: &str, documentation: &str) -> anyhow::Result<()>;
    fn put_bool(&mut self, key: &str, value: bool, documentation: &str) -> anyhow::Result<()>;
    fn put_usize(&mut self, key: &str, value: usize, documentation: &str) -> anyhow::Result<()>;
    fn put_f64(&mut self, key: &str, value: f64, documentation: &str) -> anyhow::Result<()>;

    fn create_array(
        &mut self,
        key: &str,
        documentation: &str,
        f: &mut dyn FnMut(&mut dyn TomlArrayWriter) -> anyhow::Result<()>,
    ) -> anyhow::Result<()>;

    fn put_child(
        &mut self,
        key: &str,
        f: &mut dyn FnMut(&mut dyn TomlWriter) -> anyhow::Result<()>,
    ) -> anyhow::Result<()>;
}

pub trait TomlArrayWriter {
    fn push_back_str(&mut self, value: &str) -> anyhow::Result<()>;
}
