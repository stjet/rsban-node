use crate::config::get_default_rpc_filepath;

#[no_mangle]
pub unsafe extern "C" fn rsn_get_default_rpc_filepath(buffer: *mut u8, size: usize) -> usize {
    let path = match get_default_rpc_filepath() {
        Ok(p) => p,
        Err(_) => return 0,
    };
    let buffer = std::slice::from_raw_parts_mut(buffer, size);
    let path_string = path.to_string_lossy();
    let bytes = path_string.as_bytes();
    if bytes.len() > size {
        return 0;
    }

    buffer[..bytes.len()].copy_from_slice(bytes);
    bytes.len()
}
