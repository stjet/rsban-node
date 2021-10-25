#include <nano/crypto/blake2/blake2.h>
#include <nano/lib/rsnano.hpp>
#include <nano/lib/rsnano_callbacks.hpp>
#include <nano/lib/stream.hpp>

int32_t write_u8 (void * stream, const uint8_t value)
{
	auto s{ static_cast<nano::stream *> (stream) };
	try
	{
		nano::write<uint8_t> (*s, value);
	}
	catch (...)
	{
		return -1;
	}

	return 0;
}

int32_t write_bytes (void * stream, const uint8_t * value, size_t len)
{
	auto s{ static_cast<nano::stream *> (stream) };
	try
	{
		nano::write_bytes_raw (*s, value, len);
	}
	catch (...)
	{
		return -1;
	}

	return 0;
}

int32_t read_u8 (void * stream, uint8_t * value)
{
	auto s{ static_cast<nano::stream *> (stream) };
	try
	{
		nano::read<uint8_t> (*s, *value);
	}
	catch (...)
	{
		return -1;
	}

	return 0;
}

int32_t read_bytes (void * stream, uint8_t * buffer, size_t len)
{
	auto s{ static_cast<nano::stream *> (stream) };
	try
	{
		if (nano::try_read_raw (*s, buffer, len) != 0)
		{
			return -1;
		}
	}
	catch (...)
	{
		return -1;
	}

	return 0;
}

void rsnano::set_rsnano_callbacks ()
{
	rsnano::rsn_callback_write_u8 (write_u8);
	rsnano::rsn_callback_write_bytes (write_bytes);
	rsnano::rsn_callback_read_u8 (read_u8);
	rsnano::rsn_callback_read_bytes (read_bytes);
	rsnano::rsn_callback_blake2b_init (reinterpret_cast<Blake2BInitCallback> (blake2b_init));
	rsnano::rsn_callback_blake2b_update (reinterpret_cast<Blake2BUpdateCallback> (blake2b_update));
	rsnano::rsn_callback_blake2b_final (reinterpret_cast<Blake2BFinalCallback> (blake2b_final));
}