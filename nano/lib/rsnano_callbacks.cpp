#include <nano/crypto/blake2/blake2.h>
#include <nano/lib/rsnano.hpp>
#include <nano/lib/rsnano_callbacks.hpp>
#include <nano/lib/stream.hpp>

#include <boost/property_tree/json_parser.hpp>

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

void ptree_put_string (void * ptree, const char * path, uintptr_t path_len, const char * value, uintptr_t value_len)
{
	auto tree (static_cast<boost::property_tree::ptree *> (ptree));
	std::string path_str (path, path_len);
	std::string value_str (value, value_len);
	tree->put (path_str, value);
}

int32_t ptree_get_string (const void * ptree, const char * path, uintptr_t path_len, char * result, uintptr_t result_size)
{
	try
	{
		auto tree (static_cast<const boost::property_tree::ptree *> (ptree));
		std::string path_str (path, path_len);
		auto value (tree->get<std::string> (path_str));
		if (value.length () >= result_size)
		{
			return -1;
		}

		strcpy (result, value.c_str ());
		return value.length ();
	}
	catch (...)
	{
		return -1;
	}
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
	rsnano::rsn_callback_property_tree_put_string (ptree_put_string);
	rsnano::rsn_callback_property_tree_get_string (ptree_get_string);
}