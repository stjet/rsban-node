use crate::{PublicKey, Signature, SignatureCheckSet, SignatureChecker};

#[repr(C)]
pub struct SignatureCheckSetDto {
    pub size: usize,
    pub messages: *const *const u8,
    pub message_lengths: *const usize,
    pub pub_keys: *const *const u8,
    pub signatures: *const *const u8,
    pub verifications: *mut i32,
}

pub struct SignatureCheckerHandle {
    pub checker: SignatureChecker,
}

#[no_mangle]
pub extern "C" fn rsn_signature_checker_create() -> *mut SignatureCheckerHandle {
    Box::into_raw(Box::new(SignatureCheckerHandle {
        checker: SignatureChecker::new(),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_signature_checker_destroy(handle: *mut SignatureCheckerHandle) {
    drop(Box::from_raw(handle));
}

#[no_mangle]
pub extern "C" fn rsn_signature_checker_batch_size() -> usize {
    SignatureChecker::BATCH_SIZE
}

#[no_mangle]
pub unsafe extern "C" fn rsn_signature_checker_verify(
    handle: &SignatureCheckerHandle,
    check_set: *mut SignatureCheckSetDto,
) -> bool {
    let ffi_check_set = &mut *check_set;
    let mut check_set = into_check_set(ffi_check_set);
    let result = handle.checker.verify(&mut check_set);
    let valid = std::slice::from_raw_parts_mut(ffi_check_set.verifications, ffi_check_set.size);
    valid.copy_from_slice(&check_set.verifications);
    result
}

#[no_mangle]
pub unsafe extern "C" fn rsn_signature_checker_verify_batch(
    handle: &SignatureCheckerHandle,
    check_set: *mut SignatureCheckSetDto,
    start_index: usize,
    size: usize,
) -> bool {
    let ffi_check_set = &mut *check_set;
    let mut check_set = into_check_set(ffi_check_set);

    let result = handle
        .checker
        .verify_batch(&mut check_set, start_index, size);
    let valid = std::slice::from_raw_parts_mut(ffi_check_set.verifications, ffi_check_set.size);
    valid[start_index..start_index + size]
        .copy_from_slice(&check_set.verifications[start_index..start_index + size]);
    result
}

unsafe fn into_check_set(ffi_check_set: &SignatureCheckSetDto) -> SignatureCheckSet {
    let message_lengths =
        std::slice::from_raw_parts(ffi_check_set.message_lengths, ffi_check_set.size);

    let messages: Vec<Vec<u8>> =
        std::slice::from_raw_parts(ffi_check_set.messages, ffi_check_set.size)
            .iter()
            .enumerate()
            .map(|(i, &bytes)| {
                let msg = std::slice::from_raw_parts(bytes, message_lengths[i]);
                msg.iter().cloned().collect()
            })
            .collect();

    let pub_keys = std::slice::from_raw_parts(ffi_check_set.pub_keys, ffi_check_set.size)
        .iter()
        .map(|&bytes| {
            let bytes = std::slice::from_raw_parts(bytes, 32);
            PublicKey::try_from_bytes(bytes).unwrap()
        })
        .collect();

    let signatures = std::slice::from_raw_parts(ffi_check_set.signatures, ffi_check_set.size)
        .iter()
        .map(|&bytes| {
            let bytes = std::slice::from_raw_parts(bytes, 64);
            Signature::try_from_bytes(bytes).unwrap()
        })
        .collect();

    let check_set = SignatureCheckSet::new(messages, pub_keys, signatures);
    check_set
}
