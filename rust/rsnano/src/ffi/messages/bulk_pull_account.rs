use super::{create_message_handle, create_message_handle2, MessageHandle, MessageHeaderHandle};
use crate::{ffi::NetworkConstantsDto, messages::BulkPullAccount};

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_account_create(
    constants: *mut NetworkConstantsDto,
) -> *mut MessageHandle {
    create_message_handle(constants, BulkPullAccount::new)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_message_bulk_pull_account_create2(
    header: *mut MessageHeaderHandle,
) -> *mut MessageHandle {
    create_message_handle2(header, BulkPullAccount::with_header)
}
