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

using WriteU8Callback = int32_t (*) (void *, const uint8_t *);

using ReadU8Callback = int32_t (*) (void *, uint8_t *);

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
	BlockDetailsDto details;
};

extern "C" {

void rsn_callback_write_u8 (WriteU8Callback f);

void rsn_callback_read_u8 (ReadU8Callback f);

BandwidthLimiterHandle * rsn_bandwidth_limiter_create (double limit_burst_ratio, uintptr_t limit);

void rsn_bandwidth_limiter_destroy (BandwidthLimiterHandle * limiter);

bool rsn_bandwidth_limiter_should_drop (const BandwidthLimiterHandle * limiter,
uintptr_t message_size,
int32_t * result);

int32_t rsn_bandwidth_limiter_reset (const BandwidthLimiterHandle * limiter,
double limit_burst_ratio,
uintptr_t limit);

int32_t rsn_block_details_create (uint8_t epoch,
bool is_send,
bool is_receive,
bool is_epoch,
BlockDetailsDto * result);

uint8_t rsn_block_details_packed (const BlockDetailsDto * details, int32_t * result);

int32_t rsn_block_details_unpack (uint8_t data, BlockDetailsDto * result);

int32_t rsn_block_details_serialize (const BlockDetailsDto * dto, void * stream);

int32_t rsn_block_details_deserialize (BlockDetailsDto * dto, void * stream);

void rsn_block_sideband_foo (const BlockSidebandDto * dto);

} // extern "C"

} // namespace rsnano

#endif // rs_nano_bindings_hpp
