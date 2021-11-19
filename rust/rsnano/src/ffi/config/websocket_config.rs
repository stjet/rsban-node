use super::NetworkConstantsDto;
use crate::config::{NetworkConstants, WebsocketConfig};
use std::convert::TryFrom;

#[repr(C)]
pub struct WebsocketConfigDto {
    pub enabled: bool,
    pub port: u16,
    pub address: [u8; 128],
    pub address_len: usize,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_websocket_config_create(
    dto: *mut WebsocketConfigDto,
    network: &NetworkConstantsDto,
) -> i32 {
    let network = match NetworkConstants::try_from(network) {
        Ok(n) => n,
        Err(_) => return -1,
    };
    let websocket = WebsocketConfig::new(&network);
    let dto = &mut (*dto);
    dto.enabled = websocket.enabled;
    dto.port = websocket.port;
    let bytes = websocket.address.as_bytes();
    if bytes.len() > dto.address.len() {
        return -1;
    }
    dto.address[..bytes.len()].copy_from_slice(bytes);
    dto.address_len = bytes.len();
    0
}
