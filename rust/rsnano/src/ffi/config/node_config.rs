use std::{convert::TryFrom, ffi::c_void};

use crate::{config::NodeConfig, ffi::toml::FfiToml, numbers::Amount};

#[repr(C)]
pub struct NodeConfigDto {
    pub peering_port: u16,
    pub bootstrap_fraction_numerator: u32,
    pub receive_minimum: [u8; 16],
}

#[no_mangle]
pub unsafe extern "C" fn rsn_node_config_create(dto: *mut NodeConfigDto, peering_port: u16) {
    let cfg = NodeConfig::new(peering_port);
    let dto = &mut (*dto);
    dto.peering_port = cfg.peering_port;
    dto.bootstrap_fraction_numerator = cfg.bootstrap_fraction_numerator;
    dto.receive_minimum = cfg.receive_minimum.to_be_bytes();
}

#[no_mangle]
pub extern "C" fn rsn_node_config_serialize_toml(dto: &NodeConfigDto, toml: *mut c_void) -> i32 {
    let cfg = match NodeConfig::try_from(dto) {
        Ok(c) => c,
        Err(_) => return -1,
    };
    let mut toml = FfiToml::new(toml);
    match cfg.serialize_toml(&mut toml) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

impl TryFrom<&NodeConfigDto> for NodeConfig {
    type Error = anyhow::Error;

    fn try_from(value: &NodeConfigDto) -> Result<Self, Self::Error> {
        let cfg = NodeConfig {
            peering_port: value.peering_port,
            bootstrap_fraction_numerator: value.bootstrap_fraction_numerator,
            receive_minimum: Amount::from_be_bytes(value.receive_minimum),
        };

        Ok(cfg)
    }
}
