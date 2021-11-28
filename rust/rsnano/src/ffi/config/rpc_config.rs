use super::NetworkConstantsDto;
use crate::{config::{NetworkConstants, RpcConfig}, ffi::toml::FfiToml};
use std::{convert::TryFrom, ffi::c_void};

#[repr(C)]
pub struct RpcConfigDto {
    pub address: [u8; 128],
    pub address_len: usize,
    pub port: u16,
    pub enable_control: bool,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_rpc_config_create(
    dto: *mut RpcConfigDto,
    network_constants: &NetworkConstantsDto,
) -> i32 {
    let network_constants = match NetworkConstants::try_from(network_constants) {
        Ok(nc) => nc,
        Err(_) => return -1,
    };
    let cfg = RpcConfig::new(&network_constants);
    let dto = &mut (*dto);
    fill_rpc_config_dto(dto, &cfg);
    0
}

#[no_mangle]
pub unsafe extern "C" fn rsn_rpc_config_create2(
    dto: *mut RpcConfigDto,
    network_constants: &NetworkConstantsDto,
    port: u16,
    enable_control: bool
) -> i32 {
    let network_constants = match NetworkConstants::try_from(network_constants) {
        Ok(nc) => nc,
        Err(_) => return -1,
    };
    let cfg = RpcConfig::new2(&network_constants, port, enable_control);
    let dto = &mut (*dto);
    fill_rpc_config_dto(dto, &cfg);
    0
}

fn fill_rpc_config_dto(dto: &mut RpcConfigDto, cfg: &RpcConfig) {
    let bytes = cfg.address.as_bytes();
    dto.address[..bytes.len()].copy_from_slice(bytes);
    dto.address_len = bytes.len();
    dto.port = cfg.port;
    dto.enable_control = cfg.enable_control;
}

#[no_mangle]
pub extern "C" fn rsn_rpc_config_serialize_toml(dto: &RpcConfigDto, toml: *mut c_void) -> i32 {
    let cfg = match RpcConfig::try_from(dto) {
        Ok(c) => c,
        Err(_) => return -1,
    };
    let mut toml = FfiToml::new(toml);
    match cfg.serialize_toml(&mut toml) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

impl TryFrom<&RpcConfigDto> for RpcConfig {
    type Error = anyhow::Error;

    fn try_from(dto: &RpcConfigDto) -> Result<Self, Self::Error> {
        let cfg = RpcConfig{
            address: String::from_utf8_lossy(&dto.address[..dto.address_len]).to_string(),
            port: dto.port,
            enable_control: dto.enable_control,
        };
        Ok(cfg)
    }
}