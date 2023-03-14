use std::time::Instant;

pub struct TimerHandle(pub Instant);

#[no_mangle]
pub extern "C" fn rsn_timer_create() -> *mut TimerHandle {
    Box::into_raw(Box::new(TimerHandle(Instant::now())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_timer_destroy(handle: *mut TimerHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_timer_elapsed_ms(handle: *mut TimerHandle) -> u64 {
    (*handle).0.elapsed().as_millis() as u64
}

#[no_mangle]
pub unsafe extern "C" fn rsn_timer_restart(handle: *mut TimerHandle) {
    (*handle).0 = Instant::now()
}
