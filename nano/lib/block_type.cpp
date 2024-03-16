#include <nano/lib/block_type.hpp>

void nano::serialize_block_type (nano::stream & stream, const nano::block_type & type)
{
	nano::write (stream, type);
}

