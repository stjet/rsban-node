use std::{
    ffi::{c_void, CStr},
    ops::Deref,
    path::Path,
    sync::Arc,
};

use crate::{
    datastore::lmdb::{EnvOptions, LmdbEnv},
    ffi::LmdbConfigDto,
    LmdbConfig,
};

use super::{FfiCallbacksWrapper, TransactionHandle, TransactionType};

pub struct LmdbEnvHandle(Arc<LmdbEnv>);

impl Deref for LmdbEnvHandle {
    type Target = Arc<LmdbEnv>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

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
    let env = LmdbEnv::new(&mut *error, path, &options);
    if *error {
        eprintln!("Could not create LMDB env");
    }
    Box::into_raw(Box::new(LmdbEnvHandle(Arc::new(env))))
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
    *error = (*handle).init(path, &options).is_err();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_mdb_env_close(handle: *mut LmdbEnvHandle) {
    (*handle).close();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_mdb_env_destroy(handle: *mut LmdbEnvHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_mdb_env_get_env(handle: *mut LmdbEnvHandle) -> *mut c_void {
    (*handle).0.env() as *mut c_void
}

#[no_mangle]
pub unsafe extern "C" fn rsn_mdb_env_tx_begin_read(
    handle: *mut LmdbEnvHandle,
    callbacks: *mut c_void,
) -> *mut TransactionHandle {
    let callbacks = Arc::new(FfiCallbacksWrapper::new(callbacks));
    let txn = (*handle).0.tx_begin_read(callbacks);
    TransactionHandle::new(TransactionType::Read(txn))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_mdb_env_tx_begin_write(
    handle: *mut LmdbEnvHandle,
    callbacks: *mut c_void,
) -> *mut TransactionHandle {
    let callbacks = Arc::new(FfiCallbacksWrapper::new(callbacks));
    let txn = (*handle).0.tx_begin_write(callbacks);
    TransactionHandle::new(TransactionType::Write(txn))
}
