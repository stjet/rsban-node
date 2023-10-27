#include "nano/lib/rsnano.hpp"
#include "nano/lib/rsnanoutils.hpp"

#include <nano/lib/blocks.hpp>
#include <nano/lib/stream.hpp>
#include <nano/node/bootstrap/block_deserializer.hpp>
#include <nano/node/transport/socket.hpp>

namespace
{
void block_deserialized_wrapper (void * context, rsnano::ErrorCodeDto const * ec, rsnano::BlockHandle * block_handle)
{
	auto callback = static_cast<nano::bootstrap::block_deserializer::callback_type *> (context);
	auto block = nano::block_handle_to_block (block_handle);
	auto error_code = rsnano::dto_to_error_code (*ec);
	(*callback) (error_code, block);
}

void block_deserialized_context_destroy (void * context)
{
	auto callback = static_cast<nano::bootstrap::block_deserializer::callback_type *> (context);
	delete callback;
}
}

nano::bootstrap::block_deserializer::block_deserializer (rsnano::async_runtime const & async_rt) :
	handle{ rsnano::rsn_block_deserializer_create (async_rt.handle) }
{
}

nano::bootstrap::block_deserializer::~block_deserializer ()
{
	rsnano::rsn_block_deserializer_destroy (handle);
}

void nano::bootstrap::block_deserializer::read (nano::transport::socket & socket, callback_type const && callback)
{
	debug_assert (callback);
	auto context = new nano::bootstrap::block_deserializer::callback_type (callback);
	rsnano::rsn_block_deserializer_read (handle, socket.handle, block_deserialized_wrapper, context, block_deserialized_context_destroy);
}
