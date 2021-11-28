pub trait Stream {
    fn write_u8(&mut self, value: u8) -> anyhow::Result<()>;
    fn write_bytes(&mut self, bytes: &[u8]) -> anyhow::Result<()>;
    fn read_u8(&mut self) -> anyhow::Result<u8>;
    fn read_bytes(&mut self, buffer: &mut [u8], len: usize) -> anyhow::Result<()>;
}

#[cfg(test)]
pub struct TestStream {
    bytes: Vec<u8>,
    read_index: usize,
}

#[cfg(test)]
impl TestStream {
    pub fn new() -> Self {
        Self {
            bytes: Vec::new(),
            read_index: 0,
        }
    }

    pub fn bytes_written(&self) -> usize {
        self.bytes.len()
    }

    pub fn byte_at(&self, i: usize) -> u8 {
        self.bytes[i]
    }
}

#[cfg(test)]
impl Stream for TestStream {
    fn write_u8(&mut self, value: u8) -> anyhow::Result<()> {
        self.bytes.push(value);
        Ok(())
    }

    fn write_bytes(&mut self, bytes: &[u8]) -> anyhow::Result<()> {
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    fn read_u8(&mut self) -> anyhow::Result<u8> {
        if self.read_index >= self.bytes.len() {
            bail!("no more bytes to read")
        }

        let result = self.bytes[self.read_index];
        self.read_index += 1;
        Ok(result)
    }

    fn read_bytes(&mut self, buffer: &mut [u8], len: usize) -> anyhow::Result<()> {
        if self.read_index + len > self.bytes.len() {
            bail!("not enough bytes to read")
        }

        buffer.copy_from_slice(&self.bytes[self.read_index..self.read_index + len]);
        self.read_index += len;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn test_stream() -> Result<()> {
        let mut stream = TestStream::new();
        stream.write_bytes(&[1, 2, 3])?;
        assert_eq!(stream.bytes_written(), 3);

        let mut read_buffer = [0u8; 3];
        stream.read_bytes(&mut read_buffer, 3)?;
        assert_eq!([1, 2, 3], read_buffer);

        assert!(stream.read_bytes(&mut read_buffer, 1).is_err());
        Ok(())
    }
}
