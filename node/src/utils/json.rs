use rsnano_core::utils::PropertyTree;

pub static mut CREATE_PROPERTY_TREE: Option<fn() -> Box<dyn PropertyTree + Send>> = None;

/// Note: Once FfiPropertyTree is not used anymore we can return
/// the tree unboxed
pub(crate) fn create_property_tree() -> Box<dyn PropertyTree + Send> {
    unsafe { CREATE_PROPERTY_TREE.expect("CREATE_PROPERTY_TREE missing")() }
}
