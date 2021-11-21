use crate::utils::TomlWriter;
use anyhow::Result;
use std::ffi::c_void;

type TomlPutU16Callback =
    unsafe extern "C" fn(*mut c_void, *const u8, usize, u16, *const u8, usize) -> i32;
static mut PUT_U16_CALLBACK: Option<TomlPutU16Callback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_toml_put_u16(f: TomlPutU16Callback) {
    PUT_U16_CALLBACK = Some(f);
}

pub struct FfiToml {
    handle: *mut c_void,
}

impl FfiToml {
    pub fn new(handle: *mut c_void) -> Self { Self { handle } }
}

impl TomlWriter for FfiToml {
    fn put_u16(&mut self, key: &str, value: u16, documentation: &str) -> Result<()> {
        unsafe {
            match PUT_U16_CALLBACK {
                Some(f) => {
                    if f(
                        self.handle,
                        key.as_ptr(),
                        key.bytes().len(),
                        value,
                        documentation.as_ptr(),
                        documentation.as_bytes().len(),
                    ) == 0
                    {
                        Ok(())
                    } else {
                        Err(anyhow!("PUT_U16_CALLBACK returned error"))
                    }
                }
                None => Err(anyhow!("PUT_U16_CALLBACK not set")),
            }
        }
    }
}
