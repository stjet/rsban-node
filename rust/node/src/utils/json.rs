use rsnano_core::utils::PropertyTreeWriter;

pub static mut CREATE_PROPERTY_TREE: Option<fn() -> Box<dyn PropertyTreeWriter>> = None;

/// Note: Once FfiPropertyTree is not used anymore we can return
/// the tree unboxed
pub(crate) fn create_property_tree() -> Box<dyn PropertyTreeWriter> {
    unsafe { CREATE_PROPERTY_TREE.expect("CREATE_PROPERTY_TREE missing")() }
}
