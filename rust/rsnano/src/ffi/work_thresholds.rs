use crate::WorkThresholds;

#[repr(C)]
pub struct WorkThresholdsDto {
    pub epoch_1: u64,
    pub epoch_2: u64,
    pub epoch_2_receive: u64,
    pub base: u64,
    pub entry: u64,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_work_thresholds_create(
    dto: *mut WorkThresholdsDto,
    epoch_1: u64,
    epoch_2: u64,
    epoch_2_receive: u64,
) {
    let thresholds = WorkThresholds::new(epoch_1, epoch_2, epoch_2_receive);
    fill_dto(dto, &thresholds);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_work_thresholds_publish_full(dto: *mut WorkThresholdsDto) {
    fill_dto(dto, WorkThresholds::publish_full())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_work_thresholds_publish_beta(dto: *mut WorkThresholdsDto) {
    fill_dto(dto, WorkThresholds::publish_beta())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_work_thresholds_publish_dev(dto: *mut WorkThresholdsDto) {
    fill_dto(dto, WorkThresholds::publish_dev())
}

#[no_mangle]
pub unsafe extern "C" fn rsn_work_thresholds_publish_test(dto: *mut WorkThresholdsDto) {
    fill_dto(dto, WorkThresholds::publish_test())
}

unsafe fn fill_dto(dto: *mut WorkThresholdsDto, thresholds: &WorkThresholds) {
    (*dto).epoch_1 = thresholds.epoch_1;
    (*dto).epoch_2 = thresholds.epoch_2;
    (*dto).epoch_2_receive = thresholds.epoch_2_receive;
    (*dto).base = thresholds.base;
    (*dto).entry = thresholds.entry;
}
