use crate::{ffi::NetworkConstantsDto, NetworkConstants, VotingConstants};
use std::convert::TryFrom;

#[repr(C)]
pub struct VotingConstantsDto {
    pub max_cache: usize,
    pub delay_s: i64,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_voting_constants_create(
    network_constants: &NetworkConstantsDto,
    dto: *mut VotingConstantsDto,
) -> i32 {
    let network_constants = match NetworkConstants::try_from(network_constants) {
        Ok(n) => n,
        Err(_) => return -1,
    };
    let voting = VotingConstants::new(&network_constants);
    let dto = &mut (*dto);
    fill_voting_constants_dto(dto, &voting);
    0
}

pub fn fill_voting_constants_dto(dto: &mut VotingConstantsDto, voting: &VotingConstants) {
    dto.max_cache = voting.max_cache;
    dto.delay_s = voting.delay_s;
}

impl From<&VotingConstantsDto> for VotingConstants {
    fn from(dto: &VotingConstantsDto) -> Self {
        Self {
            max_cache: dto.max_cache,
            delay_s: dto.delay_s,
        }
    }
}
