use num_traits::FromPrimitive;
use rsnano_node::transport::{
    BandwidthLimitType, OutboundBandwidthLimiter, OutboundBandwidthLimiterConfig, TrafficType,
};
use std::{ops::Deref, sync::Arc};

pub struct OutboundBandwidthLimiterHandle(pub Arc<OutboundBandwidthLimiter>);

impl Deref for OutboundBandwidthLimiterHandle {
    type Target = Arc<OutboundBandwidthLimiter>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[repr(C)]
pub struct OutboundBandwidthLimiterConfigDto {
    pub standard_limit: usize,
    pub standard_burst_ratio: f64,
    pub bootstrap_limit: usize,
    pub bootstrap_burst_ratio: f64,
}

impl From<&OutboundBandwidthLimiterConfigDto> for OutboundBandwidthLimiterConfig {
    fn from(dto: &OutboundBandwidthLimiterConfigDto) -> Self {
        Self {
            standard_limit: dto.standard_limit,
            standard_burst_ratio: dto.standard_burst_ratio,
            bootstrap_limit: dto.bootstrap_limit,
            bootstrap_burst_ratio: dto.bootstrap_burst_ratio,
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_outbound_bandwidth_limiter_destroy(
    limiter: *mut OutboundBandwidthLimiterHandle,
) {
    drop(Box::from_raw(limiter));
}

#[no_mangle]
pub extern "C" fn rsn_traffic_type_to_bandwidth_limit_type(traffic_type: u8) -> u8 {
    let traffic_type: TrafficType = FromPrimitive::from_u8(traffic_type).unwrap();
    BandwidthLimitType::from(traffic_type) as u8
}
