use crate::{
    numbers::{Amount, XRB_RATIO},
    utils::TomlWriter,
};
use anyhow::Result;

pub struct NodeConfig {
    pub peering_port: u16,
    pub bootstrap_fraction_numerator: u32,
    pub receive_minimum: Amount,
}

impl NodeConfig {
    pub fn new(peering_port: u16) -> Self {
        Self {
            peering_port,
            bootstrap_fraction_numerator: 1,
            receive_minimum: Amount::new(*XRB_RATIO),
        }
    }

    pub fn serialize_toml(&self, toml: &mut impl TomlWriter) -> Result<()> {
        toml.put_u16(
            "peering_port",
            self.peering_port,
            "Node peering port.\ntype:uint16",
        )?;
        toml.put_u32("bootstrap_fraction_numerator", self.bootstrap_fraction_numerator, "Change bootstrap threshold (online stake / 256 * bootstrap_fraction_numerator).\ntype:uint32")?;
        toml.put_str("receive_minimum", &self.receive_minimum.to_string_dec (), "Minimum receive amount. Only affects node wallets. A large amount is recommended to avoid automatic work generation for tiny transactions.\ntype:string,amount,raw")?;

        Ok(())
    }
}
