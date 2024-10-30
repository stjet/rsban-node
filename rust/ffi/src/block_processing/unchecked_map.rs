use crate::{core::UncheckedInfoHandle, utils::ContextWrapper, StatHandle, VoidPointerCallback};
use rsnano_core::{BlockHash, HashOrAccount, UncheckedInfo, UncheckedKey};
use rsnano_node::block_processing::UncheckedMap;
use std::{ffi::c_void, ops::Deref, sync::Arc};

#[repr(C)]
pub struct UncheckedKeyDto {
    pub previous: [u8; 32],
    pub hash: [u8; 32],
}

impl From<&UncheckedKeyDto> for UncheckedKey {
    fn from(dto: &UncheckedKeyDto) -> Self {
        Self {
            previous: BlockHash::from_bytes(dto.previous),
            hash: BlockHash::from_bytes(dto.hash),
        }
    }
}

impl From<&UncheckedKey> for UncheckedKeyDto {
    fn from(key: &UncheckedKey) -> Self {
        Self {
            previous: *key.previous.as_bytes(),
            hash: *key.hash.as_bytes(),
        }
    }
}

pub struct UncheckedMapHandle(pub Arc<UncheckedMap>);

impl Deref for UncheckedMapHandle {
    type Target = Arc<UncheckedMap>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_unchecked_map_create(
    max_unchecked_blocks: usize,
    stats_handle: *mut StatHandle,
    disable_delete: bool,
) -> *mut UncheckedMapHandle {
    let unchecked_map = UncheckedMap::new(
        max_unchecked_blocks,
        (*stats_handle).0.clone(),
        disable_delete,
    );
    Box::into_raw(Box::new(UncheckedMapHandle(Arc::new(unchecked_map))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_unchecked_map_destroy(handle: *mut UncheckedMapHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_unchecked_map_exists(
    handle: *mut UncheckedMapHandle,
    key: UncheckedKeyDto,
) -> bool {
    (*handle).0.exists(&UncheckedKey::from(&key))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_unchecked_map_entries_count(handle: *mut UncheckedMapHandle) -> usize {
    (*handle).0.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_unchecked_map_put(
    handle: *mut UncheckedMapHandle,
    ptr: *const u8,
    info: *mut UncheckedInfoHandle,
) {
    let dependency = HashOrAccount::from_ptr(ptr);
    (*handle).0.put(dependency, (*info).0.clone());
}

#[no_mangle]
pub unsafe extern "C" fn rsn_unchecked_map_del(
    handle: *mut UncheckedMapHandle,
    key: UncheckedKeyDto,
) {
    (*handle).0.remove(&UncheckedKey::from(&key));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_unchecked_map_clear(handle: *mut UncheckedMapHandle) {
    (*handle).0.clear();
}

pub type ActionCallback =
    unsafe extern "C" fn(*mut c_void, *mut UncheckedKeyDto, *mut UncheckedInfoHandle);

pub type PredicateCallback = unsafe extern "C" fn(*mut c_void) -> bool;

unsafe fn wrap_action_callback(
    callback: ActionCallback,
    context: *mut c_void,
    drop_context: VoidPointerCallback,
) -> Box<dyn FnMut(&UncheckedKey, &UncheckedInfo)> {
    let context_wrapper = ContextWrapper::new(context, drop_context);
    Box::new(move |k, i| {
        let key_dto = Box::into_raw(Box::new(UncheckedKeyDto::from(k)));
        let info_handle = Box::into_raw(Box::new(UncheckedInfoHandle(i.clone())));
        callback(context_wrapper.get_context(), key_dto, info_handle);
        drop(Box::from_raw(key_dto));
        drop(Box::from_raw(info_handle));
    })
}

unsafe fn wrap_predicate_callback(
    callback: PredicateCallback,
    context: *mut c_void,
    drop_context: VoidPointerCallback,
) -> Box<dyn Fn() -> bool> {
    let context_wrapper = ContextWrapper::new(context, drop_context);
    Box::new(move || callback(context_wrapper.get_context()))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_unchecked_map_for_each1(
    handle: *mut UncheckedMapHandle,
    action_callback: ActionCallback,
    action_callback_context: *mut c_void,
    drop_action_callback: VoidPointerCallback,
    predicate_callback: PredicateCallback,
    predicate_callback_context: *mut c_void,
    drop_predicate_callback: VoidPointerCallback,
) {
    let notify_observers_callback = wrap_action_callback(
        action_callback,
        action_callback_context,
        drop_action_callback,
    );

    let notify_observers_callback2 = wrap_predicate_callback(
        predicate_callback,
        predicate_callback_context,
        drop_predicate_callback,
    );

    (*handle)
        .0
        .for_each(notify_observers_callback, notify_observers_callback2);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_unchecked_map_for_each2(
    handle: *mut UncheckedMapHandle,
    dependency: *const u8,
    action_callback: ActionCallback,
    action_callback_context: *mut c_void,
    drop_action_callback: VoidPointerCallback,
    predicate_callback: PredicateCallback,
    predicate_callback_context: *mut c_void,
    drop_predicate_callback: VoidPointerCallback,
) {
    let notify_observers_callback = wrap_action_callback(
        action_callback,
        action_callback_context,
        drop_action_callback,
    );

    let notify_observers_callback2 = wrap_predicate_callback(
        predicate_callback,
        predicate_callback_context,
        drop_predicate_callback,
    );

    let mut bytes = [0; 32];
    bytes.copy_from_slice(std::slice::from_raw_parts(dependency, 32));
    (*handle).0.for_each_with_dependency(
        &HashOrAccount::from_bytes(bytes),
        notify_observers_callback,
        notify_observers_callback2,
    );
}
