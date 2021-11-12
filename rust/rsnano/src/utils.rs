#[cfg(test)]
use std::collections::HashMap;

use anyhow::Result;
use blake2::digest::{Update, VariableOutput};

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

pub trait Blake2b {
    fn init(&mut self, outlen: usize) -> Result<()>;
    fn update(&mut self, bytes: &[u8]) -> Result<()>;
    fn finalize(&mut self, out: &mut [u8]) -> Result<()>;
}

pub struct RustBlake2b {
    instance: Option<blake2::VarBlake2b>,
}

impl RustBlake2b {
    pub fn new() -> Self {
        Self { instance: None }
    }
}

impl Blake2b for RustBlake2b {
    fn init(&mut self, outlen: usize) -> Result<()> {
        self.instance = Some(blake2::VarBlake2b::new_keyed(&[], outlen));
        Ok(())
    }

    fn update(&mut self, bytes: &[u8]) -> Result<()> {
        self.instance
            .as_mut()
            .ok_or_else(|| anyhow!("not initialized"))?
            .update(bytes);
        Ok(())
    }

    fn finalize(&mut self, out: &mut [u8]) -> Result<()> {
        let i = self
            .instance
            .take()
            .ok_or_else(|| anyhow!("not initialized"))?;

        if out.len() != i.output_size() {
            return Err(anyhow!("output size does not match"));
        }

        i.finalize_variable(|bytes| {
            out.copy_from_slice(bytes);
        });
        Ok(())
    }
}

pub trait PropertyTreeReader {
    fn get_string(&self, path: &str) -> Result<String>;
}

pub trait PropertyTreeWriter {
    fn put_string(&mut self, path: &str, value: &str) -> Result<()>;
}

#[cfg(test)]
pub struct TestPropertyTree {
    properties: HashMap<String, String>,
}

#[cfg(test)]
impl TestPropertyTree {
    pub fn new() -> Self {
        Self {
            properties: HashMap::new(),
        }
    }
}

#[cfg(test)]
impl PropertyTreeReader for TestPropertyTree {
    fn get_string(&self, path: &str) -> Result<String> {
        self.properties
            .get(path)
            .cloned()
            .ok_or_else(|| anyhow!("path not found"))
    }
}

#[cfg(test)]
impl PropertyTreeWriter for TestPropertyTree {
    fn put_string(&mut self, path: &str, value: &str) -> Result<()> {
        self.properties.insert(path.to_owned(), value.to_owned());
        Ok(())
    }
}

pub struct SerdePropertyTree {
    value: serde_json::Value,
}

impl SerdePropertyTree {
    pub fn parse(s: &str) -> Result<Self> {
        Ok(Self {
            value: serde_json::from_str(s)?,
        })
    }
}

impl PropertyTreeReader for SerdePropertyTree {
    fn get_string(&self, path: &str) -> Result<String> {
        match self.value.get(path) {
            Some(v) => match v {
                serde_json::Value::String(s) => Ok(s.to_owned()),
                _ => Err(anyhow!("not a string value")),
            },
            None => Err(anyhow!("could not find path")),
        }
    }
}

pub fn seconds_since_epoch() -> u64 {
    chrono::Utc::now().timestamp() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

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

    mod property_tree {
        use super::*;

        #[test]
        fn property_not_found() {
            let tree = TestPropertyTree::new();
            assert!(tree.get_string("DoesNotExist").is_err());
        }

        #[test]
        fn set_string_property() {
            let mut tree = TestPropertyTree::new();
            tree.put_string("foo", "bar").unwrap();
            assert_eq!(tree.get_string("foo").unwrap(), "bar");
        }
    }
}
