#pragma once

#include <nano/lib/blocks.hpp>
#include <nano/secure/account_info.hpp>
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

template <typename T>
nano::store::db_val<T>::operator nano::account_info () const
{
	nano::bufferstream stream (reinterpret_cast<uint8_t const *> (data ()), size ());
	nano::account_info result;
	debug_assert (size () == result.db_size ());
	bool error = result.deserialize (stream);
	debug_assert (!error);
	return result;
}
