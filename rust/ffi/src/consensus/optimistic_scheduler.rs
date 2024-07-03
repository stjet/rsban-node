pub struct OptimisticSchedulerHandle;

#[no_mangle]
pub unsafe extern "C" fn rsn_optimistic_scheduler_destroy(handle: *mut OptimisticSchedulerHandle) {
    drop(Box::from_raw(handle))
}
