use std::{
    ffi::{c_void, CStr},
    path::Path,
    ptr,
};

use crate::{
    datastore::lmdb::{EnvOptions, LmdbEnv},
    ffi::LmdbConfigDto,
    LmdbConfig,
};

pub struct LmdbEnvHandle(LmdbEnv);

#[no_mangle]
pub unsafe extern "C" fn rsn_mdb_env_create(
    error: *mut bool,
    path: *const i8,
    lmdb_config: *const LmdbConfigDto,
    use_no_mem_init: bool,
) -> *mut LmdbEnvHandle {
    let config = LmdbConfig::from(&*lmdb_config);
    let options = EnvOptions {
        config,
        use_no_mem_init,
    };
    let path_str = CStr::from_ptr(path).to_str().unwrap();
    let path = Path::new(path_str);
    match LmdbEnv::new(path, &options) {
        Ok(env) => {
            *error = false;
            Box::into_raw(Box::new(LmdbEnvHandle(env)))
        }
        Err(e) => {
            eprintln!("Could not create LMDB env: {}", e);
            *error = true;
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_mdb_env_destroy(handle: *mut LmdbEnvHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_mdb_env_get_env(handle: *mut LmdbEnvHandle) -> *mut c_void {
    (*handle).0.environment as *mut c_void
}

#[no_mangle]
pub unsafe extern "C" fn rsn_mdb_env_close_env(handle: *mut LmdbEnvHandle) {
    (*handle).0.close_env()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_mdb_env_init(
    handle: *mut LmdbEnvHandle,
    error: *mut bool,
    path: *const i8,
    lmdb_config: *const LmdbConfigDto,
    use_no_mem_init: bool,
) {
    let config = LmdbConfig::from(&*lmdb_config);
    let options = EnvOptions {
        config,
        use_no_mem_init,
    };
    let path_str = CStr::from_ptr(path).to_str().unwrap();
    let path = Path::new(path_str);
    *error = (*handle).0.init(path, &options).is_err()
}
