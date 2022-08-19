use crate::datastore::lmdb::LmdbEnv;

pub struct LmdbEnvHandle(LmdbEnv);

#[no_mangle]
pub extern "C" fn rsn_mdb_env_create() -> *mut LmdbEnvHandle {
    Box::into_raw(Box::new(LmdbEnvHandle(LmdbEnv::new())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_mdb_env_destroy(handle: *mut LmdbEnvHandle) {
    drop(Box::from_raw(handle))
}
