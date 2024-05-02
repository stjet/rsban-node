use super::Message;
use anyhow::Result;
use std::ffi::c_void;

pub trait Listener: Send + Sync {
    fn broadcast(&self, message: &Message) -> Result<()>;
}

pub struct NullListener {}

impl NullListener {
    pub fn new() -> Self {
        Self {}
    }
}

impl Listener for NullListener {
    fn broadcast(&self, _message: &Message) -> Result<()> {
        Ok(())
    }
}

pub struct WebsocketListener {
    cpp_pointer: *mut c_void,
}

impl WebsocketListener {
    pub fn new(cpp_pointer: *mut c_void) -> Self {
        Self { cpp_pointer }
    }
}

unsafe impl Send for WebsocketListener {}
unsafe impl Sync for WebsocketListener {}

pub type BroadcastCallback = fn(*mut c_void, &Message) -> Result<()>;
pub static mut BROADCAST_CALLBACK: Option<BroadcastCallback> = None;

impl Listener for WebsocketListener {
    fn broadcast(&self, message: &Message) -> Result<()> {
        unsafe {
            match BROADCAST_CALLBACK {
                Some(f) => f(self.cpp_pointer, message),
                None => Err(anyhow!("BROADCAST_CALLBACK missing")),
            }
        }
    }
}
