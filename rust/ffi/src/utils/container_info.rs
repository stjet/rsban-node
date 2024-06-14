use crate::StringDto;
use rsnano_core::utils::{ContainerInfo, ContainerInfoComponent};
use std::ffi::{c_char, CStr};

#[repr(C)]
pub struct ContainerInfoDto {
    pub name: StringDto,
    pub count: usize,
    pub sizeof_element: usize,
}

impl From<&ContainerInfo> for ContainerInfoDto {
    fn from(value: &ContainerInfo) -> Self {
        Self {
            name: value.name.as_str().into(),
            count: value.count,
            sizeof_element: value.sizeof_element,
        }
    }
}

pub struct ContainerInfoComponentHandle(pub ContainerInfoComponent);

#[no_mangle]
pub unsafe extern "C" fn rsn_container_info_component_destroy(
    handle: *mut ContainerInfoComponentHandle,
) {
    drop(Box::from_raw(handle));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_container_info_component_is_composite(
    handle: *mut ContainerInfoComponentHandle,
) -> bool {
    match &(*handle).0 {
        ContainerInfoComponent::Leaf(_) => false,
        ContainerInfoComponent::Composite(_, _) => true,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_container_info_leaf_create(
    name: *const c_char,
    count: usize,
    sizeof_element: usize,
) -> *mut ContainerInfoComponentHandle {
    Box::into_raw(Box::new(ContainerInfoComponentHandle(
        ContainerInfoComponent::Leaf(ContainerInfo {
            name: CStr::from_ptr(name).to_str().unwrap().to_owned(),
            count,
            sizeof_element,
        }),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_container_info_leaf_get_info(
    handle: *const ContainerInfoComponentHandle,
    info: *mut ContainerInfoDto,
) {
    match &(*handle).0 {
        ContainerInfoComponent::Leaf(leaf) => (*info) = leaf.into(),
        _ => panic!("not a leaf"),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_container_info_composite_create(
    name: *const c_char,
) -> *mut ContainerInfoComponentHandle {
    let name = CStr::from_ptr(name).to_str().unwrap().to_owned();

    Box::into_raw(Box::new(ContainerInfoComponentHandle(
        ContainerInfoComponent::Composite(name, Vec::new()),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_container_info_composite_name(
    handle: *const ContainerInfoComponentHandle,
    result: *mut StringDto,
) {
    match &(*handle).0 {
        ContainerInfoComponent::Composite(name, _) => *result = name.into(),
        _ => panic!("not a composite"),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_container_info_composite_children_len(
    handle: *const ContainerInfoComponentHandle,
) -> usize {
    match &(*handle).0 {
        ContainerInfoComponent::Composite(_, children) => children.len(),
        _ => panic!("not a composite"),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_container_info_composite_child(
    handle: *const ContainerInfoComponentHandle,
    index: usize,
) -> *mut ContainerInfoComponentHandle {
    match &(*handle).0 {
        ContainerInfoComponent::Composite(_, children) => Box::into_raw(Box::new(
            ContainerInfoComponentHandle(children[index].clone()),
        )),
        _ => panic!("not a composite"),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_container_info_composite_child_add(
    handle: *mut ContainerInfoComponentHandle,
    child: *const ContainerInfoComponentHandle,
) {
    match &mut (*handle).0 {
        ContainerInfoComponent::Composite(_, children) => children.push((*child).0.clone()),
        _ => panic!("not a composite"),
    }
}
