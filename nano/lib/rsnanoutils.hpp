#pragma once

#include "nano/lib/blocks.hpp"
#include <nano/boost/asio/ip/tcp.hpp>
#include <nano/boost/asio/ip/udp.hpp>
#include <nano/lib/rsnano.hpp>
#include <nano/node/messages.hpp>

#include <cstddef>
#include <cstdint>

namespace nano
{
class message;
}

namespace rsnano
{
boost::system::error_code dto_to_error_code (rsnano::ErrorCodeDto const & dto);
rsnano::ErrorCodeDto error_code_to_dto (boost::system::error_code const & ec);
rsnano::EndpointDto udp_endpoint_to_dto (boost::asio::ip::udp::endpoint const & ep);
rsnano::EndpointDto endpoint_to_dto (boost::asio::ip::tcp::endpoint const & ep);
boost::asio::ip::tcp::endpoint dto_to_endpoint (rsnano::EndpointDto const & dto);
boost::asio::ip::udp::endpoint dto_to_udp_endpoint (rsnano::EndpointDto const & dto);
std::string convert_dto_to_string (rsnano::StringDto & dto);
std::unique_ptr<nano::message> message_handle_to_message (rsnano::MessageHandle * handle);

class io_ctx_wrapper
{
public:
	io_ctx_wrapper (boost::asio::io_context & ctx);
	io_ctx_wrapper (rsnano::IoContextHandle * handle_a);
	io_ctx_wrapper (io_ctx_wrapper const &) = delete;
	~io_ctx_wrapper ();
	rsnano::IoContextHandle * handle () const;
	boost::asio::io_context * inner () const;

private:
	rsnano::IoContextHandle * handle_m;
};

void read_block_array_dto (rsnano::BlockArrayDto & dto, std::vector<std::shared_ptr<nano::block>> & list_a);
rsnano::BlockArrayDto to_block_array_dto (std::vector<std::shared_ptr<nano::block>> & list_a);

class AtomicU64Wrapper
{
public:
	AtomicU64Wrapper (uint64_t value_a) :
		handle{ rsnano::rsn_atomic_u64_create (value_a) }
	{
	}
	AtomicU64Wrapper (AtomicU64Wrapper const &) = delete;
	AtomicU64Wrapper (AtomicU64Wrapper &&) = delete;
	~AtomicU64Wrapper ()
	{
		rsnano::rsn_atomic_u64_destroy (handle);
	}

	uint64_t load ()
	{
		return rsnano::rsn_atomic_u64_load (handle);
	}

	void store (uint64_t value_a)
	{
		return rsnano::rsn_atomic_u64_store (handle, value_a);
	}

	void add (uint64_t value_a)
	{
		return rsnano::rsn_atomic_u64_add (handle, value_a);
	}

	rsnano::AtomicU64Handle * handle;
};

class RsNanoTimer
{
public:
	RsNanoTimer () :
		handle{ rsnano::rsn_timer_create () }
	{
	}
	~RsNanoTimer ()
	{
		rsnano::rsn_timer_destroy (handle);
	}
	RsNanoTimer (RsNanoTimer const &) = delete;
	RsNanoTimer (RsNanoTimer &&) = delete;
	uint64_t elapsed_ms ()
	{
		return rsnano::rsn_timer_elapsed_ms (handle);
	}
	void restart ()
	{
		rsnano::rsn_timer_restart (handle);
	}
	rsnano::TimerHandle * handle;
};

class block_vec{
public:
	block_vec() : handle {rsnano::rsn_block_vec_create()}{}
	block_vec(block_vec const &) = delete;
	~block_vec(){rsnano::rsn_block_vec_destroy(handle);}
	void erase_last (size_t count) { rsnano::rsn_block_vec_erase_last(handle, count);}
	void push_back (nano::block const & block){rsnano::rsn_block_vec_push_back(handle, block.get_handle());}
	size_t size () const { return rsnano::rsn_block_vec_size(handle);}
	bool empty () const { return size () == 0;}
	void clear () { rsnano::rsn_block_vec_clear(handle);}
	rsnano::BlockVecHandle * handle;
};

}
