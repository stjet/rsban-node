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

struct SendBlockHandle;

using WriteU8Callback = int32_t (*) (void *, uint8_t);

using WriteBytesCallback = int32_t (*) (void *, const uint8_t *, uintptr_t);

using ReadU8Callback = int32_t (*) (void *, uint8_t *);

using ReadBytesCallback = int32_t (*) (void *, uint8_t *, uintptr_t);

struct BlockDetailsDto
{
	uint8_t epoch;
	bool is_send;
	bool is_receive;
	bool is_epoch;
};

struct BlockSidebandDto
{
	uint64_t height;
	uint64_t timestamp;
	uint8_t successor[32];
	uint8_t account[32];
	uint8_t balance[16];
	BlockDetailsDto details;
	uint8_t source_epoch;
};

struct SendHashablesDto
{
	uint8_t previous[32];
	uint8_t destination[32];
	uint8_t balance[16];
};

struct SendBlockDto
{
	SendHashablesDto hashables;
	uint8_t signature[64];
	uint64_t work;
};

extern "C" {

void rsn_callback_write_u8 (WriteU8Callback f);

void rsn_callback_write_bytes (WriteBytesCallback f);

void rsn_callback_read_u8 (ReadU8Callback f);

void rsn_callback_read_bytes (ReadBytesCallback f);

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

int32_t rsn_block_details_serialize (const BlockDetailsDto * dto, void * stream);

int32_t rsn_block_details_deserialize (BlockDetailsDto * dto, void * stream);

uintptr_t rsn_block_sideband_size (uint8_t block_type, int32_t * result);

int32_t rsn_block_sideband_serialize (const BlockSidebandDto * dto, void * stream, uint8_t block_type);

int32_t rsn_block_sideband_deserialize (BlockSidebandDto * dto, void * stream, uint8_t block_type);

int32_t rsn_send_hashables_deserialize (SendHashablesDto * dto, void * stream);

SendBlockHandle * rsn_send_block_create (const SendBlockDto * dto);

void rsn_send_block_destroy (SendBlockHandle * handle);

SendBlockHandle * rsn_send_block_clone (const SendBlockHandle * handle);

int32_t rsn_send_block_serialize (const SendBlockDto * dto, void * stream);

int32_t rsn_send_block_deserialize (SendBlockDto * dto, void * stream);

} // extern "C"

} // namespace rsnano

#endif // rs_nano_bindings_hpp
