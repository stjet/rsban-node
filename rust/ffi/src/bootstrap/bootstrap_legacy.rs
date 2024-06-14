use super::bootstrap_attempt::BootstrapAttemptHandle;
use crate::FfiPropertyTree;
use rsnano_node::bootstrap::BootstrapStrategy;
use std::ffi::c_void;

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_legacy_get_information(
    handle: &BootstrapAttemptHandle,
    ptree: *mut c_void,
) {
    let BootstrapStrategy::Legacy(legacy) = &***handle else {
        panic!("not legacy");
    };
    let mut tree = FfiPropertyTree::new_borrowed(ptree);
    legacy.get_information(&mut tree);
}
