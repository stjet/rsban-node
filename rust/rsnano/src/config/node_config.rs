use crate::utils::TomlWriter;
use anyhow::Result;

pub struct NodeConfig {
    pub peering_port: u16,
}

impl NodeConfig {
    pub fn new(peering_port: u16) -> Self {
        Self { peering_port }
    }

    pub fn serialize_toml(&self, toml: &mut impl TomlWriter) -> Result<()>{
        toml.put_u16("peering_port", self.peering_port, "Node peering port.\ntype:uint16")?;
        Ok(())
    }
}
