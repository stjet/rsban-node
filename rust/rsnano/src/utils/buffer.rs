use std::ffi::c_void;

pub trait BufferWrapper {
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn handle(&self) -> *mut c_void;
}
