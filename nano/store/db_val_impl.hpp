#include <nano/lib/blocks.hpp>
#include <nano/store/db_val.hpp>

template <typename T>
nano::store::db_val<T>::db_val (std::shared_ptr<nano::block> const & val_a) :
	buffer (std::make_shared<std::vector<uint8_t>> ())
{
	{
		nano::vectorstream stream (*buffer);
		nano::serialize_block (stream, *val_a);
	}
	convert_buffer_to_value ();
}

