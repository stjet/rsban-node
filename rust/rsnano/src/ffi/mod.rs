mod block_arrival;
mod block_processor;
mod blocks;
pub mod bootstrap;
mod config;
pub mod core;
pub mod datastore;
mod epoch;
mod hardened_constants;
mod io_context;
mod ipc;
mod logger_mt;
mod messages;
mod numbers;
mod property_tree;
mod secure;
mod signatures;
mod state_block_signature_verification;
mod stats;
mod stream;
mod thread_pool;
mod toml;
mod transport;
mod unchecked_info;
mod voting;
mod wallet;
mod websocket;

use std::{
    ffi::{c_void, CString},
    os::raw::c_char,
};

pub(crate) use block_processor::*;
pub use blocks::*;
pub use config::*;
pub use epoch::*;
pub use io_context::DispatchCallback;
pub use ipc::*;
pub use logger_mt::*;
pub use numbers::*;
pub use property_tree::*;
pub use secure::*;
pub use signatures::*;
pub use stats::*;
pub use stream::*;
pub use toml::*;
pub(crate) use unchecked_info::*;
pub(crate) use websocket::*;

use crate::{
    core::{BlockHash, PublicKey, RawKey, Signature},
    utils::ErrorCode,
    Account, Amount, HashOrAccount, MemoryIntensiveInstrumentationCallback, QualifiedRoot, Root,
    IS_SANITIZER_BUILD, MEMORY_INTENSIVE_INSTRUMENTATION,
};
pub use transport::ChannelTcpObserverWeakPtr;

pub struct StringHandle(CString);
#[repr(C)]
pub struct StringDto {
    pub handle: *mut StringHandle,
    pub value: *const c_char,
}

impl<T: AsRef<str>> From<T> for StringDto {
    fn from(s: T) -> Self {
        let handle = Box::new(StringHandle(CString::new(s.as_ref()).unwrap()));
        let value = handle.0.as_ptr();
        StringDto {
            handle: Box::into_raw(handle),
            value,
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_string_destroy(handle: *mut StringHandle) {
    drop(Box::from_raw(handle))
}

impl BlockHash {
    unsafe fn from_ptr(ptr: *const u8) -> Self {
        BlockHash::from_bytes(into_32_byte_array(ptr))
    }
}

impl Account {
    unsafe fn from_ptr(ptr: *const u8) -> Self {
        Account::from_bytes(into_32_byte_array(ptr))
    }
}

impl HashOrAccount {
    unsafe fn from_ptr(ptr: *const u8) -> Self {
        HashOrAccount::from_bytes(into_32_byte_array(ptr))
    }
}

impl Root {
    unsafe fn from_ptr(ptr: *const u8) -> Self {
        Root::from_bytes(into_32_byte_array(ptr))
    }
}

impl QualifiedRoot {
    unsafe fn from_ptr(ptr: *const u8) -> Self {
        QualifiedRoot {
            root: Root::from_ptr(ptr),
            previous: BlockHash::from_ptr(ptr.add(32)),
        }
    }
}

impl RawKey {
    unsafe fn from_ptr(ptr: *const u8) -> Self {
        RawKey::from_bytes(into_32_byte_array(ptr))
    }
}

impl Signature {
    unsafe fn from_ptr(ptr: *const u8) -> Self {
        let mut bytes = [0; 64];
        bytes.copy_from_slice(std::slice::from_raw_parts(ptr, 64));
        Signature::from_bytes(bytes)
    }
}

impl Amount {
    unsafe fn from_ptr(ptr: *const u8) -> Self {
        let mut bytes = [0; 16];
        bytes.copy_from_slice(std::slice::from_raw_parts(ptr, 16));
        Amount::from_be_bytes(bytes)
    }
}

fn into_32_byte_array(ptr: *const u8) -> [u8; 32] {
    let mut bytes = [0; 32];
    bytes.copy_from_slice(unsafe { std::slice::from_raw_parts(ptr, 32) });
    bytes
}

pub(crate) unsafe fn copy_public_key_bytes(source: &PublicKey, target: *mut u8) {
    let bytes = std::slice::from_raw_parts_mut(target, 32);
    bytes.copy_from_slice(source.as_bytes());
}

pub(crate) unsafe fn copy_raw_key_bytes(source: RawKey, target: *mut u8) {
    let bytes = std::slice::from_raw_parts_mut(target, 32);
    bytes.copy_from_slice(source.as_bytes());
}

pub(crate) unsafe fn copy_hash_bytes(source: BlockHash, target: *mut u8) {
    let bytes = std::slice::from_raw_parts_mut(target, 32);
    bytes.copy_from_slice(source.as_bytes());
}

pub(crate) unsafe fn copy_hash_or_account_bytes(source: HashOrAccount, target: *mut u8) {
    let bytes = std::slice::from_raw_parts_mut(target, 32);
    bytes.copy_from_slice(source.as_bytes());
}

pub(crate) unsafe fn copy_account_bytes(source: Account, target: *mut u8) {
    let bytes = std::slice::from_raw_parts_mut(target, 32);
    bytes.copy_from_slice(source.as_bytes());
}

pub(crate) unsafe fn copy_signature_bytes(source: &Signature, target: *mut u8) {
    let bytes = std::slice::from_raw_parts_mut(target, 64);
    bytes.copy_from_slice(source.as_bytes());
}

pub(crate) unsafe fn copy_amount_bytes(source: Amount, target: *mut u8) {
    let bytes = std::slice::from_raw_parts_mut(target, 16);
    bytes.copy_from_slice(&source.to_be_bytes());
}

pub type VoidPointerCallback = unsafe extern "C" fn(*mut c_void);

#[repr(C)]
pub struct ErrorCodeDto {
    pub val: i32,
    pub category: u8,
}

impl From<&ErrorCode> for ErrorCodeDto {
    fn from(ec: &ErrorCode) -> Self {
        Self {
            val: ec.val,
            category: ec.category,
        }
    }
}

impl From<&ErrorCodeDto> for ErrorCode {
    fn from(dto: &ErrorCodeDto) -> Self {
        Self {
            val: dto.val,
            category: dto.category,
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_memory_intensive_instrumentation(
    f: MemoryIntensiveInstrumentationCallback,
) {
    MEMORY_INTENSIVE_INSTRUMENTATION = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_is_sanitizer_build(
    f: MemoryIntensiveInstrumentationCallback,
) {
    IS_SANITIZER_BUILD = Some(f);
}

pub struct U256ArrayHandle(Box<Vec<[u8; 32]>>);

#[repr(C)]
pub struct U256ArrayDto {
    pub items: *const [u8; 32],
    pub count: usize,
    pub handle: *mut U256ArrayHandle,
}

impl U256ArrayDto {
    pub fn initialize(&mut self, values: Box<Vec<[u8; 32]>>) {
        self.items = values.as_ptr();
        self.count = values.len();
        self.handle = Box::into_raw(Box::new(U256ArrayHandle(values)))
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_u256_array_destroy(dto: *mut U256ArrayDto) {
    drop(Box::from_raw((*dto).handle))
}
