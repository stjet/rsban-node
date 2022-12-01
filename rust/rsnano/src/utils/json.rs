use anyhow::Result;
use rsnano_core::utils::{PropertyTreeReader, PropertyTreeWriter};

/// Note: Once FfiPropertyTree is not used anymore we can return
/// the tree unboxed
pub(crate) fn create_property_tree() -> Box<dyn PropertyTreeWriter> {
    crate::ffi::create_ffi_property_tree()
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
