use std::{
    ops::Deref,
    sync::{Arc, Mutex},
};

use rsnano_node::representatives::RepresentativeRegister;

pub struct RepresentativeRegisterHandle(Arc<Mutex<RepresentativeRegister>>);

impl Deref for RepresentativeRegisterHandle {
    type Target = Arc<Mutex<RepresentativeRegister>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_representative_register_create() -> *mut RepresentativeRegisterHandle {
    Box::into_raw(Box::new(RepresentativeRegisterHandle(Arc::new(
        Mutex::new(RepresentativeRegister::new()),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_representative_register_destroy(
    handle: *mut RepresentativeRegisterHandle,
) {
    drop(Box::from_raw(handle))
}
