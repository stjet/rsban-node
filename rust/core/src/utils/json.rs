use std::any::Any;
use std::collections::HashMap;

use serde_json::{Map, Value};

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

pub struct TestPropertyTree {
    properties: HashMap<String, String>,
}

impl TestPropertyTree {
    pub fn new() -> Self {
        Self {
            properties: HashMap::new(),
        }
    }
}

impl PropertyTreeReader for TestPropertyTree {
    fn get_string(&self, path: &str) -> anyhow::Result<String> {
        self.properties
            .get(path)
            .cloned()
            .ok_or_else(|| anyhow!("path not found"))
    }
}

impl PropertyTreeWriter for TestPropertyTree {
    fn put_string(&mut self, path: &str, value: &str) -> anyhow::Result<()> {
        self.properties.insert(path.to_owned(), value.to_owned());
        Ok(())
    }

    fn put_u64(&mut self, _path: &str, _value: u64) -> anyhow::Result<()> {
        todo!()
    }

    fn new_writer(&self) -> Box<dyn PropertyTreeWriter> {
        todo!()
    }

    fn push_back(&mut self, _path: &str, _value: &dyn PropertyTreeWriter) {
        todo!()
    }

    fn add_child(&mut self, _path: &str, _value: &dyn PropertyTreeWriter) {
        todo!()
    }

    fn add(&mut self, _path: &str, _value: &str) -> anyhow::Result<()> {
        todo!()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clear(&mut self) -> anyhow::Result<()> {
        todo!()
    }

    fn put_child(&mut self, _path: &str, _value: &dyn PropertyTreeWriter) {
        todo!()
    }

    fn to_json(&self) -> String {
        todo!()
    }
}

pub struct SerdePropertyTree {
    value: Value,
}

impl SerdePropertyTree {
    pub fn new() -> Self {
        Self {
            value: Value::Object(Map::new()),
        }
    }
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        Ok(Self {
            value: serde_json::from_str(s)?,
        })
    }
}

impl PropertyTreeReader for SerdePropertyTree {
    fn get_string(&self, path: &str) -> anyhow::Result<String> {
        match self.value.get(path) {
            Some(v) => match v {
                serde_json::Value::String(s) => Ok(s.to_owned()),
                _ => Err(anyhow!("not a string value")),
            },
            None => Err(anyhow!("could not find path")),
        }
    }
}

impl PropertyTreeWriter for SerdePropertyTree {
    fn clear(&mut self) -> anyhow::Result<()> {
        self.value = Value::Object(Map::new());
        Ok(())
    }

    fn put_string(&mut self, path: &str, value: &str) -> anyhow::Result<()> {
        let Value::Object(map) = &mut self.value else {bail!("not an object")};
        map.insert(path.to_string(), Value::String(value.to_string()));
        Ok(())
    }

    fn put_u64(&mut self, path: &str, value: u64) -> anyhow::Result<()> {
        let Value::Object(map) = &mut self.value else {bail!("not an object")};
        map.insert(path.to_string(), Value::Number(value.into()));
        Ok(())
    }

    fn new_writer(&self) -> Box<dyn PropertyTreeWriter> {
        Box::new(Self::new())
    }

    fn push_back(&mut self, _path: &str, _value: &dyn PropertyTreeWriter) {
        todo!()
    }

    fn add_child(&mut self, _path: &str, _value: &dyn PropertyTreeWriter) {
        todo!()
    }

    fn put_child(&mut self, _path: &str, _value: &dyn PropertyTreeWriter) {
        todo!()
    }

    fn add(&mut self, _path: &str, _value: &str) -> anyhow::Result<()> {
        todo!()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn to_json(&self) -> String {
        self.value.to_string()
    }
}

#[cfg(test)]
mod tests {
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
