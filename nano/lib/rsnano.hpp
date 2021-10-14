#ifndef rs_nano_bindings_hpp
#define rs_nano_bindings_hpp

#include <cstdarg>
#include <cstdint>
#include <cstdlib>
#include <new>
#include <ostream>

namespace rsnano
{
struct BandwidthLimiterHandle;

struct BlockDetailsDto
{
	uint8_t epoch;
	bool is_send;
	bool is_receive;
	bool is_epoch;
};

extern "C" {

BandwidthLimiterHandle * rsn_bandwidth_limiter_create (double limit_burst_ratio, uintptr_t limit);

void rsn_bandwidth_limiter_destroy (BandwidthLimiterHandle * limiter);

bool rsn_bandwidth_limiter_should_drop (const BandwidthLimiterHandle * limiter,
uintptr_t message_size);

void rsn_bandwidth_limiter_reset (const BandwidthLimiterHandle * limiter,
double limit_burst_ration,
uintptr_t limit);

void block_details_create (uint8_t epoch,
bool is_send,
bool is_receive,
bool is_epoch,
BlockDetailsDto * result);

uint8_t block_details_packed (const BlockDetailsDto * details);

void block_details_unpack (uint8_t data, BlockDetailsDto * result);

} // extern "C"

} // namespace rsnano

#endif // rs_nano_bindings_hpp
