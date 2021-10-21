#include <nano/lib/stream.hpp>

void nano::write_bytes_raw (nano::stream & stream_a, uint8_t const * bytes_a, size_t len_a)
{
	auto amount_written (stream_a.sputn (bytes_a, len_a));
	(void)amount_written;
	debug_assert (amount_written == sizeof (len_a));
}
