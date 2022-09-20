#include <nano/secure/versioning.hpp>

#include <boost/endian/conversion.hpp>

#include <lmdb/libraries/liblmdb/lmdb.h>

nano::block_sideband_v18::block_sideband_v18 (nano::account const & account_a, nano::block_hash const & successor_a, nano::amount const & balance_a, uint64_t height_a, uint64_t timestamp_a, nano::block_details const & details_a) :
	successor (successor_a),
	account (account_a),
	balance (balance_a),
	height (height_a),
	timestamp (timestamp_a),
	details (details_a)
{
}

nano::block_sideband_v18::block_sideband_v18 (nano::account const & account_a, nano::block_hash const & successor_a, nano::amount const & balance_a, uint64_t height_a, uint64_t timestamp_a, nano::epoch epoch_a, bool is_send, bool is_receive, bool is_epoch) :
	successor (successor_a),
	account (account_a),
	balance (balance_a),
	height (height_a),
	timestamp (timestamp_a),
	details (epoch_a, is_send, is_receive, is_epoch)
{
}

size_t nano::block_sideband_v18::size (nano::block_type type_a)
{
	size_t result (0);
	result += sizeof (successor);
	if (type_a != nano::block_type::state && type_a != nano::block_type::open)
	{
		result += sizeof (account);
	}
	if (type_a != nano::block_type::open)
	{
		result += sizeof (height);
	}
	if (type_a == nano::block_type::receive || type_a == nano::block_type::change || type_a == nano::block_type::open)
	{
		result += sizeof (balance);
	}
	result += sizeof (timestamp);
	if (type_a == nano::block_type::state)
	{
		static_assert (sizeof (nano::epoch) == nano::block_details::size (), "block_details_v18 is larger than the epoch enum");
		result += nano::block_details::size ();
	}
	return result;
}

void nano::block_sideband_v18::serialize (nano::stream & stream_a, nano::block_type type_a) const
{
	nano::write (stream_a, successor.bytes);
	if (type_a != nano::block_type::state && type_a != nano::block_type::open)
	{
		nano::write (stream_a, account.bytes);
	}
	if (type_a != nano::block_type::open)
	{
		nano::write (stream_a, boost::endian::native_to_big (height));
	}
	if (type_a == nano::block_type::receive || type_a == nano::block_type::change || type_a == nano::block_type::open)
	{
		nano::write (stream_a, balance.bytes);
	}
	nano::write (stream_a, boost::endian::native_to_big (timestamp));
	if (type_a == nano::block_type::state)
	{
		details.serialize (stream_a);
	}
}

bool nano::block_sideband_v18::deserialize (nano::stream & stream_a, nano::block_type type_a)
{
	bool result (false);
	try
	{
		nano::read (stream_a, successor.bytes);
		if (type_a != nano::block_type::state && type_a != nano::block_type::open)
		{
			nano::read (stream_a, account.bytes);
		}
		if (type_a != nano::block_type::open)
		{
			nano::read (stream_a, height);
			boost::endian::big_to_native_inplace (height);
		}
		else
		{
			height = 1;
		}
		if (type_a == nano::block_type::receive || type_a == nano::block_type::change || type_a == nano::block_type::open)
		{
			nano::read (stream_a, balance.bytes);
		}
		nano::read (stream_a, timestamp);
		boost::endian::big_to_native_inplace (timestamp);
		if (type_a == nano::block_type::state)
		{
			result = details.deserialize (stream_a);
		}
	}
	catch (std::runtime_error &)
	{
		result = true;
	}

	return result;
}
