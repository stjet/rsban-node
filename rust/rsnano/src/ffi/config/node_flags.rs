use std::{
    ops::Deref,
    sync::{Arc, Mutex},
};

use crate::NodeFlags;

pub struct NodeFlagsHandle(Arc<Mutex<NodeFlags>>);

impl NodeFlagsHandle {
    pub fn new(flags: Arc<Mutex<NodeFlags>>) -> *mut NodeFlagsHandle {
        Box::into_raw(Box::new(NodeFlagsHandle(flags)))
    }
}

impl Deref for NodeFlagsHandle {
    type Target = Arc<Mutex<NodeFlags>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_node_flags_create() -> *mut NodeFlagsHandle {
    NodeFlagsHandle::new(Arc::new(Mutex::new(NodeFlags::new())))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_node_flags_destroy(handle: *mut NodeFlagsHandle) {
    drop(Box::from_raw(handle))
}
