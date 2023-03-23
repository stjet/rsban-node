use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

pub struct TimerHandle(pub Arc<Mutex<Instant>>);

#[no_mangle]
pub extern "C" fn rsn_timer_create() -> *mut TimerHandle {
    Box::into_raw(Box::new(TimerHandle(Arc::new(Mutex::new(Instant::now())))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_timer_destroy(handle: *mut TimerHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_timer_elapsed_ms(handle: *mut TimerHandle) -> u64 {
    (*handle).0.lock().unwrap().elapsed().as_millis() as u64
}

#[no_mangle]
pub unsafe extern "C" fn rsn_timer_restart(handle: *mut TimerHandle) {
    *(*handle).0.lock().unwrap() = Instant::now()
}
