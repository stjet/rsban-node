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

struct BlockSidebandDto
{
	uint8_t source_epoch;
	uint64_t height;
	uint64_t timestamp;
};

extern "C" {

BandwidthLimiterHandle * rsn_bandwidth_limiter_create (double limit_burst_ratio, uintptr_t limit);

void rsn_bandwidth_limiter_destroy (BandwidthLimiterHandle * limiter);

bool rsn_bandwidth_limiter_should_drop (const BandwidthLimiterHandle * limiter,
uintptr_t message_size);

void rsn_bandwidth_limiter_reset (const BandwidthLimiterHandle * limiter,
double limit_burst_ration,
uintptr_t limit);

void rsn_block_details_create (uint8_t epoch,
bool is_send,
bool is_receive,
bool is_epoch,
BlockDetailsDto * result);

uint8_t rsn_block_details_packed (const BlockDetailsDto * details);

void rsn_block_details_unpack (uint8_t data, BlockDetailsDto * result);

void rsn_block_sideband_foo (const BlockSidebandDto * dto);

} // extern "C"

} // namespace rsnano

#endif // rs_nano_bindings_hpp
