#include <nano/lib/rsnano.hpp>
#include <nano/lib/rsnano_callbacks.hpp>
#include <nano/lib/stream.hpp>

int32_t write_u8 (void * stream, const uint8_t * value)
{
	auto s = static_cast<nano::stream *> (stream);
	try
	{
		nano::write<uint8_t> (*s, *value);
	}
	catch (...)
	{
		return -1;
	}

	return 0;
}

int32_t read_u8 (void * stream, uint8_t * value)
{
	auto s = static_cast<nano::stream *> (stream);
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

void rsnano::set_rsnano_callbacks ()
{
	rsnano::rsn_callback_write_u8 (write_u8);
	rsnano::rsn_callback_read_u8 (read_u8);
}