pub struct HintedSchedulerHandle;

#[no_mangle]
pub unsafe extern "C" fn rsn_hinted_scheduler_destroy(handle: *mut HintedSchedulerHandle) {
    drop(Box::from_raw(handle));
}
