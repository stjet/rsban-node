use std::ffi::c_void;

pub trait BufferWrapper: Send + Sync {
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn handle(&self) -> *mut c_void;
    fn get_slice_mut(&self) -> &mut [u8];
}
