#include <nano/crypto/blake2/blake2.h>
#include <nano/lib/logger_mt.hpp>
#include <nano/lib/rsnano.hpp>
#include <nano/lib/stream.hpp>
#include <nano/lib/tomlconfig.hpp>
#include <nano/node/blockprocessor.hpp>
#include <nano/node/rsnano_callbacks.hpp>
#include <nano/node/websocket.hpp>

#include <boost/property_tree/json_parser.hpp>

#include <iostream>

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

size_t in_avail (void * stream, int32_t * error)
{
	auto s{ static_cast<nano::stream *> (stream) };
	try
	{
		*error = 0;
		return s->in_avail ();
	}
	catch (...)
	{
		*error = 1;
		return 0;
	}
}

void ptree_put_string (void * ptree, const char * path, uintptr_t path_len, const char * value, uintptr_t value_len)
{
	auto tree (static_cast<boost::property_tree::ptree *> (ptree));
	std::string path_str (path, path_len);
	std::string value_str (value, value_len);
	tree->put (path_str, value_str);
}

void ptree_put_u64 (void * ptree, const char * path, uintptr_t path_len, uint64_t value)
{
	auto tree (static_cast<boost::property_tree::ptree *> (ptree));
	std::string path_str (path, path_len);
	tree->put (path_str, value);
}

void ptree_add (void * ptree, const char * path, uintptr_t path_len, const char * value, uintptr_t value_len)
{
	auto tree (static_cast<boost::property_tree::ptree *> (ptree));
	std::string path_str (path, path_len);
	std::string value_str (value, value_len);
	tree->add (path_str, value_str);
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

void * ptree_create ()
{
	return new boost::property_tree::ptree ();
}

void ptree_destroy (void * handle)
{
	delete static_cast<boost::property_tree::ptree *> (handle);
}

void ptree_push_back (void * parent_handle, const char * name, const void * child_handle)
{
	auto parent_tree{ static_cast<boost::property_tree::ptree *> (parent_handle) };
	auto child_tree{ static_cast<const boost::property_tree::ptree *> (child_handle) };
	std::string name_l (name);
	parent_tree->push_back (std::make_pair (name_l, *child_tree));
}

void ptree_add_child (void * parent_handle, const char * name, const void * child_handle)
{
	auto parent_tree{ static_cast<boost::property_tree::ptree *> (parent_handle) };
	auto child_tree{ static_cast<const boost::property_tree::ptree *> (child_handle) };
	std::string name_l (name);
	parent_tree->add_child (name_l, *child_tree);
}

int32_t toml_put_u64 (void * toml_a, const uint8_t * key_a, uintptr_t key_len_a, uint64_t value, const uint8_t * documentation_a, uintptr_t documentation_len_a)
{
	try
	{
		auto toml{ static_cast<nano::tomlconfig *> (toml_a) };
		std::string key (reinterpret_cast<const char *> (key_a), key_len_a);
		std::string documentation (reinterpret_cast<const char *> (documentation_a), documentation_len_a);
		toml->put (key, value, documentation.c_str ());
	}
	catch (...)
	{
		return -1;
	}

	return 0;
}

int32_t toml_put_i64 (void * toml_a, const uint8_t * key_a, uintptr_t key_len_a, int64_t value, const uint8_t * documentation_a, uintptr_t documentation_len_a)
{
	try
	{
		auto toml{ static_cast<nano::tomlconfig *> (toml_a) };
		std::string key (reinterpret_cast<const char *> (key_a), key_len_a);
		std::string documentation (reinterpret_cast<const char *> (documentation_a), documentation_len_a);
		toml->put (key, value, documentation.c_str ());
	}
	catch (...)
	{
		return -1;
	}

	return 0;
}

int32_t toml_put_f64 (void * toml_a, const uint8_t * key_a, uintptr_t key_len_a, double value, const uint8_t * documentation_a, uintptr_t documentation_len_a)
{
	try
	{
		auto toml{ static_cast<nano::tomlconfig *> (toml_a) };
		std::string key (reinterpret_cast<const char *> (key_a), key_len_a);
		std::string documentation (reinterpret_cast<const char *> (documentation_a), documentation_len_a);
		toml->put (key, value, documentation.c_str ());
	}
	catch (...)
	{
		return -1;
	}

	return 0;
}

int32_t toml_put_str (void * toml_a, const uint8_t * key_a, uintptr_t key_len_a, const uint8_t * value_a, uintptr_t value_len_a, const uint8_t * documentation_a, uintptr_t documentation_len_a)
{
	try
	{
		auto toml{ static_cast<nano::tomlconfig *> (toml_a) };
		std::string key (reinterpret_cast<const char *> (key_a), key_len_a);
		std::string documentation (reinterpret_cast<const char *> (documentation_a), documentation_len_a);
		std::string value (reinterpret_cast<const char *> (value_a), value_len_a);
		toml->put (key, value, documentation.c_str ());
	}
	catch (...)
	{
		return -1;
	}

	return 0;
}

int32_t toml_put_bool (void * toml_a, const uint8_t * key_a, uintptr_t key_len_a, bool value, const uint8_t * documentation_a, uintptr_t documentation_len_a)
{
	try
	{
		auto toml{ static_cast<nano::tomlconfig *> (toml_a) };
		std::string key (reinterpret_cast<const char *> (key_a), key_len_a);
		std::string documentation (reinterpret_cast<const char *> (documentation_a), documentation_len_a);
		toml->put (key, value, documentation.c_str ());
	}
	catch (...)
	{
		return -1;
	}

	return 0;
}

struct TomlArrayHandle
{
	TomlArrayHandle (std::shared_ptr<cpptoml::array> ptr_a) :
		ptr{ ptr_a }
	{
	}
	std::shared_ptr<cpptoml::array> ptr;
};

void * toml_create_array (void * toml_a, const uint8_t * key_a, uintptr_t key_len_a, const uint8_t * documentation_a, uintptr_t documentation_len_a)
{
	try
	{
		auto toml{ static_cast<nano::tomlconfig *> (toml_a) };
		std::string key (reinterpret_cast<const char *> (key_a), key_len_a);
		std::string documentation (reinterpret_cast<const char *> (documentation_a), documentation_len_a);
		auto arr{ toml->create_array (key, documentation.c_str ()) };
		auto result = new TomlArrayHandle (arr);
		return result;
	}
	catch (...)
	{
		return nullptr;
	}
}

void toml_drop_array (void * handle_a)
{
	auto handle = reinterpret_cast<TomlArrayHandle *> (handle_a);
	delete handle;
}

void toml_array_put_str (void * handle_a, const uint8_t * value_a, uintptr_t value_len)
{
	auto handle = reinterpret_cast<TomlArrayHandle *> (handle_a);
	std::string val (reinterpret_cast<const char *> (value_a), value_len);
	handle->ptr->push_back (val);
}

void * toml_create_config ()
{
	return new nano::tomlconfig ();
}

void toml_drop_config (void * handle)
{
	delete static_cast<nano::tomlconfig *> (handle);
}

void toml_put_child (void * handle_a, const uint8_t * key_a, uintptr_t key_len_a, void * child_a)
{
	auto parent = static_cast<nano::tomlconfig *> (handle_a);
	auto child = static_cast<nano::tomlconfig *> (child_a);
	std::string key (reinterpret_cast<const char *> (key_a), key_len_a);
	parent->put_child (key, *child);
}

bool logger_try_log (void * handle_a, const uint8_t * message_a, size_t len_a)
{
	auto logger = static_cast<nano::logger_mt *> (handle_a);
	auto message_string = std::string (reinterpret_cast<const char *> (message_a), len_a);
	return logger->try_log (message_string);
}

void logger_always_log (void * handle_a, const uint8_t * message_a, size_t len_a)
{
	auto logger = static_cast<nano::logger_mt *> (handle_a);
	auto message_string = std::string (reinterpret_cast<const char *> (message_a), len_a);
	return logger->always_log (message_string);
}

bool listener_broadcast (void * handle_a, rsnano::MessageDto const * message_a)
{
	try
	{
		auto ptree = static_cast<boost::property_tree::ptree const *> (message_a->contents);
		nano::websocket::message message_l (static_cast<nano::websocket::topic> (message_a->topic));
		message_l.contents = *ptree;

		auto listener = static_cast<nano::websocket::listener *> (handle_a);
		listener->broadcast (message_l);
		return true;
	}
	catch (...)
	{
		return false;
	}
}

void blockprocessor_add (void * handle_a, rsnano::UncheckedInfoHandle * info_a)
{
	auto processor = static_cast<nano::block_processor *> (handle_a);
	nano::unchecked_info info{ info_a };
	processor->add (info);
}

static bool callbacks_set = false;

void rsnano::set_rsnano_callbacks ()
{
	if (callbacks_set)
		return;

	rsnano::rsn_callback_write_u8 (write_u8);
	rsnano::rsn_callback_write_bytes (write_bytes);
	rsnano::rsn_callback_read_u8 (read_u8);
	rsnano::rsn_callback_read_bytes (read_bytes);
	rsnano::rsn_callback_in_avail (in_avail);
	rsnano::rsn_callback_blake2b_init (reinterpret_cast<Blake2BInitCallback> (blake2b_init));
	rsnano::rsn_callback_blake2b_update (reinterpret_cast<Blake2BUpdateCallback> (blake2b_update));
	rsnano::rsn_callback_blake2b_final (reinterpret_cast<Blake2BFinalCallback> (blake2b_final));
	rsnano::rsn_callback_property_tree_put_string (ptree_put_string);
	rsnano::rsn_callback_property_tree_put_u64 (ptree_put_u64);
	rsnano::rsn_callback_property_tree_add (ptree_add);
	rsnano::rsn_callback_property_tree_get_string (ptree_get_string);
	rsnano::rsn_callback_property_tree_create (ptree_create);
	rsnano::rsn_callback_property_tree_destroy (ptree_destroy);
	rsnano::rsn_callback_property_tree_push_back (ptree_push_back);
	rsnano::rsn_callback_property_tree_add_child (ptree_add_child);
	rsnano::rsn_callback_toml_put_u64 (toml_put_u64);
	rsnano::rsn_callback_toml_put_i64 (toml_put_i64);
	rsnano::rsn_callback_toml_put_str (toml_put_str);
	rsnano::rsn_callback_toml_put_bool (toml_put_bool);
	rsnano::rsn_callback_toml_put_f64 (toml_put_f64);
	rsnano::rsn_callback_toml_create_array (toml_create_array);
	rsnano::rsn_callback_toml_array_put_str (toml_array_put_str);
	rsnano::rsn_callback_toml_create_config (toml_create_config);
	rsnano::rsn_callback_toml_drop_config (toml_drop_config);
	rsnano::rsn_callback_toml_put_child (toml_put_child);
	rsnano::rsn_callback_toml_drop_array (toml_drop_array);
	rsnano::rsn_callback_try_log (logger_try_log);
	rsnano::rsn_callback_always_log (logger_always_log);
	rsnano::rsn_callback_listener_broadcast (listener_broadcast);
	rsnano::rsn_callback_block_processor_add (blockprocessor_add);
	callbacks_set = true;
}