pub trait Stream {
    fn write_u8(&mut self, value: u8) -> anyhow::Result<()>;
    fn read_u8(&mut self) -> anyhow::Result<u8>;
}
