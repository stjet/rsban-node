#include <nano/crypto_lib/random_pool.hpp>
#include <nano/lib/blocks.hpp>
#include <nano/lib/memory.hpp>
#include <nano/lib/numbers.hpp>
#include <nano/lib/threading.hpp>
#include <nano/secure/common.hpp>

#include <crypto/cryptopp/words.h>

#include <boost/endian/conversion.hpp>
#include <boost/property_tree/json_parser.hpp>

#include <bitset>

/** Compare blocks, first by type, then content. This is an optimization over dynamic_cast, which is very slow on some platforms. */
namespace
{
template <typename T>
bool blocks_equal (T const & first, nano::block const & second)
{
	static_assert (std::is_base_of<nano::block, T>::value, "Input parameter is not a block type");
	return (first.type () == second.type ()) && (static_cast<T const &> (second)) == first;
}

template <typename block>
std::shared_ptr<block> deserialize_block (nano::stream & stream_a)
{
	auto error (false);
	auto result = nano::make_shared<block> (error, stream_a);
	if (error)
	{
		result = nullptr;
	}

	return result;
}
}

void nano::block_memory_pool_purge ()
{
	nano::purge_shared_ptr_singleton_pool_memory<nano::open_block> ();
	nano::purge_shared_ptr_singleton_pool_memory<nano::state_block> ();
	nano::purge_shared_ptr_singleton_pool_memory<nano::send_block> ();
	nano::purge_shared_ptr_singleton_pool_memory<nano::change_block> ();
}

std::string nano::block::to_json () const
{
	std::string result;
	serialize_json (result);
	return result;
}

size_t nano::block::size (nano::block_type type_a)
{
	return rsnano::rsn_block_serialized_size (static_cast<uint8_t> (type_a));
}

nano::work_version nano::block::work_version () const
{
	return nano::work_version::work_1;
}

void nano::block::refresh ()
{
	if (!cached_hash.is_zero ())
	{
		cached_hash = generate_hash ();
	}
}

nano::block_hash const & nano::block::hash () const
{
	if (!cached_hash.is_zero ())
	{
		// Once a block is created, it should not be modified (unless using refresh ())
		// This would invalidate the cache; check it hasn't changed.
		debug_assert (cached_hash == generate_hash ());
	}
	else
	{
		cached_hash = generate_hash ();
	}

	return cached_hash;
}

nano::block_hash nano::block::full_hash () const
{
	nano::block_hash result;
	blake2b_state state;
	blake2b_init (&state, sizeof (result.bytes));
	blake2b_update (&state, hash ().bytes.data (), sizeof (hash ()));
	auto signature (block_signature ());
	blake2b_update (&state, signature.bytes.data (), sizeof (signature));
	auto work (block_work ());
	blake2b_update (&state, &work, sizeof (work));
	blake2b_final (&state, result.bytes.data (), sizeof (result.bytes));
	return result;
}

nano::block_sideband nano::block::sideband () const
{
	rsnano::BlockSidebandDto dto;
	auto block_dto{ to_block_dto () };
	if (rsnano::rsn_block_sideband (&block_dto, &dto) < 0)
		throw std::runtime_error ("cannot get sideband");
	return nano::block_sideband (dto);
}

void nano::block::sideband_set (nano::block_sideband const & sideband_a)
{
	auto block_dto{ to_block_dto () };
	if (rsnano::rsn_block_sideband_set (&block_dto, &sideband_a.as_dto ()) < 0)
		throw std::runtime_error ("cannot set sideband");
}

bool nano::block::has_sideband () const
{
	auto block_dto{ to_block_dto () };
	return rsnano::rsn_block_has_sideband (&block_dto);
}

nano::account nano::block::representative () const
{
	static nano::account representative{};
	return representative;
}

nano::block_hash nano::block::source () const
{
	static nano::block_hash source{ 0 };
	return source;
}

nano::account nano::block::destination () const
{
	static nano::account destination{};
	return destination;
}

nano::link nano::block::link () const
{
	static nano::link link{ 0 };
	return link;
}

nano::account nano::block::account () const
{
	static nano::account account{};
	return account;
}

nano::qualified_root nano::block::qualified_root () const
{
	return nano::qualified_root (root (), previous ());
}

nano::amount nano::block::balance () const
{
	static nano::amount amount{ 0 };
	return amount;
}

rsnano::SharedBlockEnumHandle * nano::block::to_shared_handle () const
{
	auto dto (to_block_dto ());
	return rsnano::rsn_block_to_shared_handle (&dto);
}

void nano::send_block::visit (nano::block_visitor & visitor_a) const
{
	visitor_a.send_block (*this);
}

void nano::send_block::visit (nano::mutable_block_visitor & visitor_a)
{
	visitor_a.send_block (*this);
}

uint64_t nano::send_block::block_work () const
{
	return rsnano::rsn_send_block_work (handle);
}

void nano::send_block::block_work_set (uint64_t work_a)
{
	rsnano::rsn_send_block_work_set (handle, work_a);
}

void nano::send_block::zero ()
{
	rsnano::rsn_send_block_zero (handle);
}

void nano::send_block::set_destination (nano::account account_a)
{
	uint8_t bytes[32];
	std::copy (std::begin (account_a.bytes), std::end (account_a.bytes), std::begin (bytes));
	rsnano::rsn_send_block_destination_set (handle, &bytes);
}

void nano::send_block::previous_set (nano::block_hash previous_a)
{
	uint8_t bytes[32];
	std::copy (std::begin (previous_a.bytes), std::end (previous_a.bytes), std::begin (bytes));
	rsnano::rsn_send_block_previous_set (handle, &bytes);
}

void nano::send_block::balance_set (nano::amount balance_a)
{
	uint8_t bytes[16];
	std::copy (std::begin (balance_a.bytes), std::end (balance_a.bytes), std::begin (bytes));
	rsnano::rsn_send_block_balance_set (handle, &bytes);
}

void nano::send_block::sign_zero ()
{
	uint8_t sig[64]{ 0 };
	rsnano::rsn_send_block_signature_set (handle, &sig);
}

void nano::send_block::serialize (nano::stream & stream_a) const
{
	if (rsnano::rsn_send_block_serialize (handle, &stream_a) != 0)
	{
		throw std::runtime_error ("could not serialize send_block");
	}
}

void nano::send_block::serialize_json (std::string & string_a, bool single_line) const
{
	boost::property_tree::ptree tree;
	serialize_json (tree);
	std::stringstream ostream;
	boost::property_tree::write_json (ostream, tree, !single_line);
	string_a = ostream.str ();
}

void nano::send_block::serialize_json (boost::property_tree::ptree & tree) const
{
	if (rsnano::rsn_send_block_serialize_json (handle, &tree) < 0)
		throw std::runtime_error ("could not serialize send_block as JSON");
}

nano::send_block::send_block (nano::block_hash const & previous_a, nano::account const & destination_a, nano::amount const & balance_a, nano::raw_key const & prv_a, nano::public_key const & pub_a, uint64_t work_a)
{
	debug_assert (destination_a != nullptr);
	debug_assert (pub_a != nullptr);

	rsnano::SendBlockDto2 dto;
	std::copy (std::begin (previous_a.bytes), std::end (previous_a.bytes), std::begin (dto.previous));
	std::copy (std::begin (destination_a.bytes), std::end (destination_a.bytes), std::begin (dto.destination));
	std::copy (std::begin (balance_a.bytes), std::end (balance_a.bytes), std::begin (dto.balance));
	std::copy (std::begin (prv_a.bytes), std::end (prv_a.bytes), std::begin (dto.priv_key));
	std::copy (std::begin (pub_a.bytes), std::end (pub_a.bytes), std::begin (dto.pub_key));
	dto.work = work_a;
	handle = rsnano::rsn_send_block_create2 (&dto);
	if (handle == nullptr)
		throw std::runtime_error ("could not create send_block");
}

nano::send_block::send_block (bool & error_a, nano::stream & stream_a)
{
	handle = rsnano::rsn_send_block_deserialize (&stream_a);
	error_a = handle == nullptr;
}

nano::send_block::send_block (bool & error_a, boost::property_tree::ptree const & tree_a)
{
	handle = rsnano::rsn_send_block_deserialize_json (&tree_a);
	error_a = handle == nullptr;
}

rsnano::SendBlockDto empty_send_block_dto ()
{
	rsnano::SendBlockDto dto;
	std::fill (std::begin (dto.signature), std::end (dto.signature), 0);
	std::fill (std::begin (dto.previous), std::end (dto.previous), 0);
	std::fill (std::begin (dto.destination), std::end (dto.destination), 0);
	std::fill (std::begin (dto.balance), std::end (dto.balance), 0);
	dto.work = 0;
	return dto;
}

nano::send_block::send_block ()
{
	auto dto{ empty_send_block_dto () };
	handle = rsnano::rsn_send_block_create (&dto);
}

nano::send_block::send_block (const send_block & other)
{
	cached_hash = other.cached_hash;
	if (other.handle == nullptr)
	{
		handle = nullptr;
	}
	else
	{
		handle = rsnano::rsn_send_block_clone (other.handle);
	}
}

nano::send_block::send_block (send_block && other)
{
	cached_hash = other.cached_hash;
	handle = other.handle;
	other.handle = nullptr;
}

nano::send_block::send_block (rsnano::SendBlockHandle * handle_a) :
	handle (handle_a)
{
}

nano::send_block::~send_block ()
{
	if (handle != nullptr)
	{
		rsnano::rsn_send_block_destroy (handle);
		handle = nullptr;
	}
}

bool nano::send_block::operator== (nano::block const & other_a) const
{
	return blocks_equal (*this, other_a);
}

bool nano::send_block::valid_predecessor (nano::block const & block_a) const
{
	return rsnano::rsn_send_block_valid_predecessor (static_cast<uint8_t> (block_a.type ()));
}

nano::block_type nano::send_block::type () const
{
	return nano::block_type::send;
}

bool nano::send_block::operator== (nano::send_block const & other_a) const
{
	return rsnano::rsn_send_block_equals (handle, other_a.handle);
}

nano::block_hash nano::send_block::previous () const
{
	uint8_t buffer[32];
	rsnano::rsn_send_block_previous (handle, &buffer);
	nano::block_hash result;
	std::copy (std::begin (buffer), std::end (buffer), std::begin (result.bytes));
	return result;
}

nano::account nano::send_block::destination () const
{
	uint8_t buffer[32];
	rsnano::rsn_send_block_destination (handle, &buffer);
	nano::account result;
	std::copy (std::begin (buffer), std::end (buffer), std::begin (result.bytes));
	return result;
}

nano::root nano::send_block::root () const
{
	return previous ();
}

nano::amount nano::send_block::balance () const
{
	uint8_t buffer[16];
	rsnano::rsn_send_block_balance (handle, &buffer);
	nano::amount result;
	std::copy (std::begin (buffer), std::end (buffer), std::begin (result.bytes));
	return result;
}

nano::signature nano::send_block::block_signature () const
{
	uint8_t bytes[64];
	rsnano::rsn_send_block_signature (handle, &bytes);
	nano::signature result;
	std::copy (std::begin (bytes), std::end (bytes), std::begin (result.bytes));
	return result;
}

void nano::send_block::signature_set (nano::signature const & signature_a)
{
	uint8_t bytes[64];
	std::copy (std::begin (signature_a.bytes), std::end (signature_a.bytes), std::begin (bytes));
	rsnano::rsn_send_block_signature_set (handle, &bytes);
}

std::size_t nano::send_block::size ()
{
	return sizeof (nano::block_hash) + sizeof (nano::account) + sizeof (nano::amount) + sizeof (nano::signature) + sizeof (uint64_t);
}

nano::block_hash nano::send_block::generate_hash () const
{
	uint8_t bytes[32];
	rsnano::rsn_send_block_hash (handle, &bytes);
	nano::block_hash result;
	std::copy (std::begin (bytes), std::end (bytes), std::begin (result.bytes));
	return result;
}

rsnano::BlockDto nano::send_block::to_block_dto () const
{
	rsnano::BlockDto dto;
	dto.block_type = static_cast<uint8_t> (nano::block_type::send);
	dto.handle = handle;
	return dto;
}

void * nano::send_block::get_handle () const
{
	return handle;
}

nano::open_block::open_block ()
{
	rsnano::OpenBlockDto dto;
	dto.work = 0;
	std::fill (std::begin (dto.account), std::end (dto.account), 0);
	std::fill (std::begin (dto.source), std::end (dto.source), 0);
	std::fill (std::begin (dto.representative), std::end (dto.representative), 0);
	std::fill (std::begin (dto.signature), std::end (dto.signature), 0);
	handle = rsnano::rsn_open_block_create (&dto);
}

nano::open_block::open_block (nano::block_hash const & source_a, nano::account const & representative_a, nano::account const & account_a, nano::raw_key const & prv_a, nano::public_key const & pub_a, uint64_t work_a)
{
	debug_assert (representative_a != nullptr);
	debug_assert (account_a != nullptr);
	debug_assert (pub_a != nullptr);

	rsnano::OpenBlockDto2 dto;
	std::copy (std::begin (source_a.bytes), std::end (source_a.bytes), std::begin (dto.source));
	std::copy (std::begin (representative_a.bytes), std::end (representative_a.bytes), std::begin (dto.representative));
	std::copy (std::begin (account_a.bytes), std::end (account_a.bytes), std::begin (dto.account));
	std::copy (std::begin (prv_a.bytes), std::end (prv_a.bytes), std::begin (dto.priv_key));
	std::copy (std::begin (pub_a.bytes), std::end (pub_a.bytes), std::begin (dto.pub_key));
	dto.work = work_a;
	handle = rsnano::rsn_open_block_create2 (&dto);
	if (handle == nullptr)
		throw std::runtime_error ("could not create open_block");
}

nano::open_block::open_block (nano::block_hash const & source_a, nano::account const & representative_a, nano::account const & account_a, std::nullptr_t)
{
	debug_assert (representative_a != nullptr);
	debug_assert (account_a != nullptr);

	rsnano::OpenBlockDto dto;
	dto.work = 0;
	std::copy (std::begin (source_a.bytes), std::end (source_a.bytes), std::begin (dto.source));
	std::copy (std::begin (representative_a.bytes), std::end (representative_a.bytes), std::begin (dto.representative));
	std::copy (std::begin (account_a.bytes), std::end (account_a.bytes), std::begin (dto.account));
	std::fill (std::begin (dto.signature), std::end (dto.signature), 0);
	handle = rsnano::rsn_open_block_create (&dto);
}

nano::open_block::open_block (bool & error_a, nano::stream & stream_a)
{
	handle = rsnano::rsn_open_block_deserialize (&stream_a);
	error_a = handle == nullptr;
}

nano::open_block::open_block (bool & error_a, boost::property_tree::ptree const & tree_a)
{
	handle = rsnano::rsn_open_block_deserialize_json (&tree_a);
	error_a = handle == nullptr;
}

nano::open_block::open_block (const open_block & other)
{
	cached_hash = other.cached_hash;
	if (other.handle == nullptr)
	{
		handle = nullptr;
	}
	else
	{
		handle = rsnano::rsn_open_block_clone (other.handle);
	}
}

nano::open_block::open_block (nano::open_block && other)
{
	cached_hash = other.cached_hash;
	handle = other.handle;
	other.handle = nullptr;
}

nano::open_block::open_block (rsnano::OpenBlockHandle * handle_a) :
	handle (handle_a)
{
}

nano::open_block::~open_block ()
{
	if (handle != nullptr)
	{
		rsnano::rsn_open_block_destroy (handle);
		handle = nullptr;
	}
}

uint64_t nano::open_block::block_work () const
{
	return rsnano::rsn_open_block_work (handle);
}

void nano::open_block::block_work_set (uint64_t work_a)
{
	rsnano::rsn_open_block_work_set (handle, work_a);
}

nano::block_hash nano::open_block::previous () const
{
	static nano::block_hash result{ 0 };
	return result;
}

nano::account nano::open_block::account () const
{
	uint8_t buffer[32];
	rsnano::rsn_open_block_account (handle, &buffer);
	nano::account result;
	std::copy (std::begin (buffer), std::end (buffer), std::begin (result.bytes));
	return result;
}

void nano::open_block::serialize (nano::stream & stream_a) const
{
	if (rsnano::rsn_open_block_serialize (handle, &stream_a) != 0)
	{
		throw std::runtime_error ("open_block serialization failed");
	}
}

void nano::open_block::serialize_json (std::string & string_a, bool single_line) const
{
	boost::property_tree::ptree tree;
	serialize_json (tree);
	std::stringstream ostream;
	boost::property_tree::write_json (ostream, tree, !single_line);
	string_a = ostream.str ();
}

void nano::open_block::serialize_json (boost::property_tree::ptree & tree) const
{
	if (rsnano::rsn_open_block_serialize_json (handle, &tree) < 0)
		throw std::runtime_error ("could not JSON serialize open_block");
}

void nano::open_block::visit (nano::block_visitor & visitor_a) const
{
	visitor_a.open_block (*this);
}

void nano::open_block::visit (nano::mutable_block_visitor & visitor_a)
{
	visitor_a.open_block (*this);
}

nano::block_type nano::open_block::type () const
{
	return nano::block_type::open;
}

bool nano::open_block::operator== (nano::block const & other_a) const
{
	return blocks_equal (*this, other_a);
}

bool nano::open_block::operator== (nano::open_block const & other_a) const
{
	return rsnano::rsn_open_block_equals (handle, other_a.handle);
}

bool nano::open_block::valid_predecessor (nano::block const & block_a) const
{
	return false;
}

nano::block_hash nano::open_block::source () const
{
	uint8_t buffer[32];
	rsnano::rsn_open_block_source (handle, &buffer);
	nano::block_hash result;
	std::copy (std::begin (buffer), std::end (buffer), std::begin (result.bytes));
	return result;
}

nano::root nano::open_block::root () const
{
	return account ();
}

nano::account nano::open_block::representative () const
{
	uint8_t buffer[32];
	rsnano::rsn_open_block_representative (handle, &buffer);
	nano::account result;
	std::copy (std::begin (buffer), std::end (buffer), std::begin (result.bytes));
	return result;
}

nano::signature nano::open_block::block_signature () const
{
	uint8_t buffer[64];
	rsnano::rsn_open_block_signature (handle, &buffer);
	nano::signature result;
	std::copy (std::begin (buffer), std::end (buffer), std::begin (result.bytes));
	return result;
}

void nano::open_block::signature_set (nano::signature const & signature_a)
{
	uint8_t buffer[64];
	std::copy (std::begin (signature_a.bytes), std::end (signature_a.bytes), std::begin (buffer));
	rsnano::rsn_open_block_signature_set (handle, &buffer);
}

void nano::open_block::sign_zero ()
{
	uint8_t buffer[64];
	std::fill (std::begin (buffer), std::end (buffer), 0);
	rsnano::rsn_open_block_signature_set (handle, &buffer);
}

void nano::open_block::source_set (nano::block_hash source_a)
{
	uint8_t buffer[32];
	std::copy (std::begin (source_a.bytes), std::end (source_a.bytes), std::begin (buffer));
	rsnano::rsn_open_block_source_set (handle, &buffer);
}

void nano::open_block::account_set (nano::account account_a)
{
	uint8_t buffer[32];
	std::copy (std::begin (account_a.bytes), std::end (account_a.bytes), std::begin (buffer));
	rsnano::rsn_open_block_account_set (handle, &buffer);
}

void nano::open_block::representative_set (nano::account account_a)
{
	uint8_t buffer[32];
	std::copy (std::begin (account_a.bytes), std::end (account_a.bytes), std::begin (buffer));
	rsnano::rsn_open_block_representative_set (handle, &buffer);
}

void nano::open_block::zero ()
{
	block_work_set (0);
	sign_zero ();
	account_set (0);
	representative_set (0);
	source_set (0);
}

std::size_t nano::open_block::size ()
{
	return rsnano::rsn_open_block_size ();
}

nano::block_hash nano::open_block::generate_hash () const
{
	uint8_t bytes[32];
	rsnano::rsn_open_block_hash (handle, &bytes);
	nano::block_hash result;
	std::copy (std::begin (bytes), std::end (bytes), std::begin (result.bytes));
	return result;
}

rsnano::BlockDto nano::open_block::to_block_dto () const
{
	rsnano::BlockDto dto;
	dto.block_type = static_cast<uint8_t> (nano::block_type::open);
	dto.handle = handle;
	return dto;
}

void * nano::open_block::get_handle () const
{
	return handle;
}

nano::change_block::change_block ()
{
	rsnano::ChangeBlockDto dto;
	std::fill (std::begin (dto.previous), std::end (dto.previous), 0);
	std::fill (std::begin (dto.representative), std::end (dto.representative), 0);
	std::fill (std::begin (dto.signature), std::end (dto.signature), 0);
	dto.work = 0;
	handle = rsnano::rsn_change_block_create (&dto);
}

nano::change_block::change_block (nano::block_hash const & previous_a, nano::account const & representative_a, nano::raw_key const & prv_a, nano::public_key const & pub_a, uint64_t work_a)
{
	debug_assert (representative_a != nullptr);
	debug_assert (pub_a != nullptr);

	rsnano::ChangeBlockDto2 dto;
	std::copy (std::begin (previous_a.bytes), std::end (previous_a.bytes), std::begin (dto.previous));
	std::copy (std::begin (representative_a.bytes), std::end (representative_a.bytes), std::begin (dto.representative));
	std::copy (std::begin (prv_a.bytes), std::end (prv_a.bytes), std::begin (dto.priv_key));
	std::copy (std::begin (pub_a.bytes), std::end (pub_a.bytes), std::begin (dto.pub_key));
	dto.work = work_a;

	handle = rsnano::rsn_change_block_create2 (&dto);
	if (handle == nullptr)
		throw std::runtime_error ("could not create change_block");
}

nano::change_block::change_block (bool & error_a, nano::stream & stream_a)
{
	handle = rsnano::rsn_change_block_deserialize (&stream_a);
	error_a = handle == nullptr;
}

nano::change_block::change_block (bool & error_a, boost::property_tree::ptree const & tree_a)
{
	handle = rsnano::rsn_change_block_deserialize_json (&tree_a);
	error_a = handle == nullptr;
}

nano::change_block::change_block (const nano::change_block & other_a)
{
	cached_hash = other_a.cached_hash;
	if (other_a.handle == nullptr)
	{
		handle = nullptr;
	}
	else
	{
		handle = rsnano::rsn_change_block_clone (other_a.handle);
	}
}

nano::change_block::change_block (nano::change_block && other_a)
{
	cached_hash = other_a.cached_hash;
	handle = other_a.handle;
	other_a.handle = nullptr;
}

nano::change_block::change_block (rsnano::ChangeBlockHandle * handle_a) :
	handle (handle_a)
{
}

nano::change_block::~change_block ()
{
	if (handle != nullptr)
	{
		rsnano::rsn_change_block_destroy (handle);
		handle = nullptr;
	}
}

uint64_t nano::change_block::block_work () const
{
	return rsnano::rsn_change_block_work (handle);
}

void nano::change_block::block_work_set (uint64_t work_a)
{
	rsnano::rsn_change_block_work_set (handle, work_a);
}

nano::block_hash nano::change_block::previous () const
{
	uint8_t buffer[32];
	rsnano::rsn_change_block_previous (handle, &buffer);
	nano::block_hash result;
	std::copy (std::begin (buffer), std::end (buffer), std::begin (result.bytes));
	return result;
}

void nano::change_block::serialize (nano::stream & stream_a) const
{
	if (rsnano::rsn_change_block_serialize (handle, &stream_a) != 0)
	{
		throw std::runtime_error ("could not serialize change_block");
	}
}

void nano::change_block::serialize_json (std::string & string_a, bool single_line) const
{
	boost::property_tree::ptree tree;
	serialize_json (tree);
	std::stringstream ostream;
	boost::property_tree::write_json (ostream, tree, !single_line);
	string_a = ostream.str ();
}

void nano::change_block::serialize_json (boost::property_tree::ptree & tree) const
{
	if (rsnano::rsn_change_block_serialize_json (handle, &tree) < 0)
		throw std::runtime_error ("could not JSON serialize change_block");
}

void nano::change_block::visit (nano::block_visitor & visitor_a) const
{
	visitor_a.change_block (*this);
}

void nano::change_block::visit (nano::mutable_block_visitor & visitor_a)
{
	visitor_a.change_block (*this);
}

nano::block_type nano::change_block::type () const
{
	return nano::block_type::change;
}

bool nano::change_block::operator== (nano::block const & other_a) const
{
	return blocks_equal (*this, other_a);
}

bool nano::change_block::operator== (nano::change_block const & other_a) const
{
	return rsnano::rsn_change_block_equals (handle, other_a.handle);
}

bool nano::change_block::valid_predecessor (nano::block const & block_a) const
{
	bool result;
	switch (block_a.type ())
	{
		case nano::block_type::send:
		case nano::block_type::receive:
		case nano::block_type::open:
		case nano::block_type::change:
			result = true;
			break;
		default:
			result = false;
			break;
	}
	return result;
}

nano::root nano::change_block::root () const
{
	return previous ();
}

nano::account nano::change_block::representative () const
{
	uint8_t buffer[32];
	rsnano::rsn_change_block_representative (handle, &buffer);
	nano::account result;
	std::copy (std::begin (buffer), std::end (buffer), std::begin (result.bytes));
	return result;
}

nano::signature nano::change_block::block_signature () const
{
	uint8_t buffer[64];
	rsnano::rsn_change_block_signature (handle, &buffer);
	nano::signature result;
	std::copy (std::begin (buffer), std::end (buffer), std::begin (result.bytes));
	return result;
}

void nano::change_block::signature_set (nano::signature const & signature_a)
{
	uint8_t buffer[64];
	std::copy (std::begin (signature_a.bytes), std::end (signature_a.bytes), std::begin (buffer));
	rsnano::rsn_change_block_signature_set (handle, &buffer);
}

void nano::change_block::previous_set (nano::block_hash previous_a)
{
	uint8_t buffer[32];
	std::copy (std::begin (previous_a.bytes), std::end (previous_a.bytes), std::begin (buffer));
	rsnano::rsn_change_block_previous_set (handle, &buffer);
}

void nano::change_block::representative_set (nano::account account_a)
{
	uint8_t buffer[32];
	std::copy (std::begin (account_a.bytes), std::end (account_a.bytes), std::begin (buffer));
	rsnano::rsn_change_block_representative_set (handle, &buffer);
}

void nano::change_block::sign_zero ()
{
	signature_set (nano::signature (0));
}

void nano::change_block::zero ()
{
	block_work_set (0);
	sign_zero ();
	previous_set (0);
	representative_set (0);
}

std::size_t nano::change_block::size ()
{
	return rsnano::rsn_change_block_size ();
}

nano::block_hash nano::change_block::generate_hash () const
{
	uint8_t bytes[32];
	rsnano::rsn_change_block_hash (handle, &bytes);
	nano::block_hash result;
	std::copy (std::begin (bytes), std::end (bytes), std::begin (result.bytes));
	return result;
}

rsnano::BlockDto nano::change_block::to_block_dto () const
{
	rsnano::BlockDto dto;
	dto.block_type = static_cast<uint8_t> (nano::block_type::change);
	dto.handle = handle;
	return dto;
}

void * nano::change_block::get_handle () const
{
	return handle;
}

nano::state_block::state_block ()
{
	rsnano::StateBlockDto dto;
	dto.work = 0;
	std::fill (std::begin (dto.account), std::end (dto.account), 0);
	std::fill (std::begin (dto.previous), std::end (dto.previous), 0);
	std::fill (std::begin (dto.representative), std::end (dto.representative), 0);
	std::fill (std::begin (dto.balance), std::end (dto.balance), 0);
	std::fill (std::begin (dto.link), std::end (dto.link), 0);
	std::fill (std::begin (dto.signature), std::end (dto.signature), 0);
	handle = rsnano::rsn_state_block_create (&dto);
}

nano::state_block::state_block (nano::account const & account_a, nano::block_hash const & previous_a, nano::account const & representative_a, nano::amount const & balance_a, nano::link const & link_a, nano::raw_key const & prv_a, nano::public_key const & pub_a, uint64_t work_a)
{
	debug_assert (account_a != nullptr);
	debug_assert (representative_a != nullptr);
	debug_assert (link_a.as_account () != nullptr);
	debug_assert (pub_a != nullptr);

	rsnano::StateBlockDto2 dto;
	std::copy (std::begin (account_a.bytes), std::end (account_a.bytes), std::begin (dto.account));
	std::copy (std::begin (previous_a.bytes), std::end (previous_a.bytes), std::begin (dto.previous));
	std::copy (std::begin (representative_a.bytes), std::end (representative_a.bytes), std::begin (dto.representative));
	std::copy (std::begin (link_a.bytes), std::end (link_a.bytes), std::begin (dto.link));
	std::copy (std::begin (balance_a.bytes), std::end (balance_a.bytes), std::begin (dto.balance));
	std::copy (std::begin (prv_a.bytes), std::end (prv_a.bytes), std::begin (dto.priv_key));
	std::copy (std::begin (pub_a.bytes), std::end (pub_a.bytes), std::begin (dto.pub_key));
	dto.work = work_a;
	handle = rsnano::rsn_state_block_create2 (&dto);
	if (handle == nullptr)
		throw std::runtime_error ("could not create state_block");
}

nano::state_block::state_block (bool & error_a, nano::stream & stream_a)
{
	handle = rsnano::rsn_state_block_deserialize (&stream_a);
	error_a = handle == nullptr;
}

nano::state_block::state_block (bool & error_a, boost::property_tree::ptree const & tree_a)
{
	handle = rsnano::rsn_state_block_deserialize_json (&tree_a);
	error_a = handle == nullptr;
}

nano::state_block::state_block (const nano::state_block & other)
{
	cached_hash = other.cached_hash;
	if (other.handle == nullptr)
	{
		handle = nullptr;
	}
	else
	{
		handle = rsnano::rsn_state_block_clone (other.handle);
	}
}

nano::state_block::state_block (nano::state_block && other)
{
	cached_hash = other.cached_hash;
	handle = other.handle;
	other.handle = nullptr;
}

nano::state_block::state_block (rsnano::StateBlockHandle * handle_a) :
	handle (handle_a)
{
}

nano::state_block::~state_block ()
{
	if (handle != nullptr)
	{
		rsnano::rsn_state_block_destroy (handle);
		handle = nullptr;
	}
}

uint64_t nano::state_block::block_work () const
{
	return rsnano::rsn_state_block_work (handle);
}

void nano::state_block::block_work_set (uint64_t work_a)
{
	rsnano::rsn_state_block_work_set (handle, work_a);
}

nano::block_hash nano::state_block::previous () const
{
	uint8_t buffer[32];
	rsnano::rsn_state_block_previous (handle, &buffer);
	nano::block_hash result;
	std::copy (std::begin (buffer), std::end (buffer), std::begin (result.bytes));
	return result;
}

nano::account nano::state_block::account () const
{
	uint8_t buffer[32];
	rsnano::rsn_state_block_account (handle, &buffer);
	nano::account result;
	std::copy (std::begin (buffer), std::end (buffer), std::begin (result.bytes));
	return result;
}

void nano::state_block::serialize (nano::stream & stream_a) const
{
	if (rsnano::rsn_state_block_serialize (handle, &stream_a) != 0)
	{
		throw std::runtime_error ("could not serialize state_block");
	}
}

void nano::state_block::serialize_json (std::string & string_a, bool single_line) const
{
	boost::property_tree::ptree tree;
	serialize_json (tree);
	std::stringstream ostream;
	boost::property_tree::write_json (ostream, tree, !single_line);
	string_a = ostream.str ();
}

void nano::state_block::serialize_json (boost::property_tree::ptree & tree) const
{
	if (rsnano::rsn_state_block_serialize_json (handle, &tree) < 0)
		throw std::runtime_error ("could not JSON serialize state_block");
}

void nano::state_block::visit (nano::block_visitor & visitor_a) const
{
	visitor_a.state_block (*this);
}

void nano::state_block::visit (nano::mutable_block_visitor & visitor_a)
{
	visitor_a.state_block (*this);
}

nano::block_type nano::state_block::type () const
{
	return nano::block_type::state;
}

bool nano::state_block::operator== (nano::block const & other_a) const
{
	return blocks_equal (*this, other_a);
}

bool nano::state_block::operator== (nano::state_block const & other_a) const
{
	return rsnano::rsn_state_block_equals (handle, other_a.handle);
}

nano::state_block & nano::state_block::operator= (const nano::state_block & other)
{
	cached_hash = other.cached_hash;
	if (other.handle == nullptr)
	{
		handle = nullptr;
	}
	else
	{
		handle = rsnano::rsn_state_block_clone (other.handle);
	}
	return *this;
}

bool nano::state_block::valid_predecessor (nano::block const & block_a) const
{
	return true;
}

nano::root nano::state_block::root () const
{
	if (!previous ().is_zero ())
	{
		return previous ();
	}
	else
	{
		return account ();
	}
}

nano::link nano::state_block::link () const
{
	uint8_t buffer[32];
	rsnano::rsn_state_block_link (handle, &buffer);
	nano::link result;
	std::copy (std::begin (buffer), std::end (buffer), std::begin (result.bytes));
	return result;
}

nano::account nano::state_block::representative () const
{
	uint8_t buffer[32];
	rsnano::rsn_state_block_representative (handle, &buffer);
	nano::account result;
	std::copy (std::begin (buffer), std::end (buffer), std::begin (result.bytes));
	return result;
}

nano::amount nano::state_block::balance () const
{
	uint8_t buffer[16];
	rsnano::rsn_state_block_balance (handle, &buffer);
	nano::amount result;
	std::copy (std::begin (buffer), std::end (buffer), std::begin (result.bytes));
	return result;
}

nano::signature nano::state_block::block_signature () const
{
	uint8_t buffer[64];
	rsnano::rsn_state_block_signature (handle, &buffer);
	nano::signature result;
	std::copy (std::begin (buffer), std::end (buffer), std::begin (result.bytes));
	return result;
}

void nano::state_block::signature_set (nano::signature const & signature_a)
{
	uint8_t buffer[64];
	std::copy (std::begin (signature_a.bytes), std::end (signature_a.bytes), std::begin (buffer));
	rsnano::rsn_state_block_signature_set (handle, &buffer);
}

void nano::state_block::previous_set (nano::block_hash previous_a)
{
	uint8_t buffer[32];
	std::copy (std::begin (previous_a.bytes), std::end (previous_a.bytes), std::begin (buffer));
	rsnano::rsn_state_block_previous_set (handle, &buffer);
}

void nano::state_block::balance_set (nano::amount balance_a)
{
	uint8_t buffer[16];
	std::copy (std::begin (balance_a.bytes), std::end (balance_a.bytes), std::begin (buffer));
	rsnano::rsn_state_block_balance_set (handle, &buffer);
}

void nano::state_block::account_set (nano::account account_a)
{
	uint8_t buffer[32];
	std::copy (std::begin (account_a.bytes), std::end (account_a.bytes), std::begin (buffer));
	rsnano::rsn_state_block_account_set (handle, &buffer);
}

void nano::state_block::representative_set (nano::account account_a)
{
	uint8_t buffer[32];
	std::copy (std::begin (account_a.bytes), std::end (account_a.bytes), std::begin (buffer));
	rsnano::rsn_state_block_representative_set (handle, &buffer);
}

void nano::state_block::link_set (nano::link link)
{
	uint8_t buffer[32];
	std::copy (std::begin (link.bytes), std::end (link.bytes), std::begin (buffer));
	rsnano::rsn_state_block_link_set (handle, &buffer);
}

void nano::state_block::sign_zero ()
{
	nano::signature sig (0);
	signature_set (sig);
}

void nano::state_block::zero ()
{
	sign_zero ();
	block_work_set (0);
	account_set (0);
	previous_set (0);
	representative_set (0);
	balance_set (0);
	link_set (0);
}

std::size_t nano::state_block::size ()
{
	return rsnano::rsn_state_block_size ();
}

nano::block_hash nano::state_block::generate_hash () const
{
	uint8_t bytes[32];
	rsnano::rsn_state_block_hash (handle, &bytes);
	nano::block_hash result;
	std::copy (std::begin (bytes), std::end (bytes), std::begin (result.bytes));
	return result;
}

rsnano::BlockDto nano::state_block::to_block_dto () const
{
	rsnano::BlockDto dto;
	dto.block_type = static_cast<uint8_t> (nano::block_type::state);
	dto.handle = handle;
	return dto;
}

void * nano::state_block::get_handle () const
{
	return handle;
}

rsnano::BlockDto nano::block_to_block_dto (nano::block const & source)
{
	rsnano::BlockDto result;
	result.block_type = static_cast<uint8_t> (source.type ());
	result.handle = source.get_handle ();
	return result;
}

std::shared_ptr<nano::block> nano::block_dto_to_block (rsnano::BlockDto const & dto)
{
	std::unique_ptr<nano::block> obj;
	switch (static_cast<nano::block_type> (dto.block_type))
	{
		case nano::block_type::send:
			obj = std::make_unique<nano::send_block> (static_cast<rsnano::SendBlockHandle *> (dto.handle));
			break;
		case nano::block_type::receive:
			obj = std::make_unique<nano::receive_block> (static_cast<rsnano::ReceiveBlockHandle *> (dto.handle));
			break;
		case nano::block_type::open:
			obj = std::make_unique<nano::open_block> (static_cast<rsnano::OpenBlockHandle *> (dto.handle));
			break;
		case nano::block_type::change:
			obj = std::make_unique<nano::change_block> (static_cast<rsnano::ChangeBlockHandle *> (dto.handle));
			break;
		case nano::block_type::state:
			obj = std::make_unique<nano::state_block> (static_cast<rsnano::StateBlockHandle *> (dto.handle));
			break;
		default:
			break;
	}

	std::shared_ptr<nano::block> result;
	result = std::move (obj);
	return result;
}

std::shared_ptr<nano::block> nano::deserialize_block_json (boost::property_tree::ptree const & tree_a, nano::block_uniquer * uniquer_a)
{
	std::shared_ptr<nano::block> result;
	rsnano::BlockDto dto;
	if (rsnano::rsn_deserialize_block_json (&dto, &tree_a) < 0)
		return result;

	result = nano::block_dto_to_block (dto);
	if (uniquer_a != nullptr)
	{
		result = uniquer_a->unique (result);
	}
	return result;
}

std::shared_ptr<nano::block> nano::deserialize_block (nano::stream & stream_a)
{
	nano::block_type type;
	auto error (try_read (stream_a, type));
	std::shared_ptr<nano::block> result;
	if (!error)
	{
		result = nano::deserialize_block (stream_a, type);
	}
	return result;
}

std::shared_ptr<nano::block> nano::deserialize_block (nano::stream & stream_a, nano::block_type type_a, nano::block_uniquer * uniquer_a)
{
	std::shared_ptr<nano::block> result;
	switch (type_a)
	{
		case nano::block_type::receive:
		{
			result = ::deserialize_block<nano::receive_block> (stream_a);
			break;
		}
		case nano::block_type::send:
		{
			result = ::deserialize_block<nano::send_block> (stream_a);
			break;
		}
		case nano::block_type::open:
		{
			result = ::deserialize_block<nano::open_block> (stream_a);
			break;
		}
		case nano::block_type::change:
		{
			result = ::deserialize_block<nano::change_block> (stream_a);
			break;
		}
		case nano::block_type::state:
		{
			result = ::deserialize_block<nano::state_block> (stream_a);
			break;
		}
		default:
#ifndef NANO_FUZZER_TEST
			debug_assert (false);
#endif
			break;
	}
	if (uniquer_a != nullptr)
	{
		result = uniquer_a->unique (result);
	}
	return result;
}

void nano::receive_block::visit (nano::block_visitor & visitor_a) const
{
	visitor_a.receive_block (*this);
}

void nano::receive_block::visit (nano::mutable_block_visitor & visitor_a)
{
	visitor_a.receive_block (*this);
}

bool nano::receive_block::operator== (nano::receive_block const & other_a) const
{
	return rsnano::rsn_receive_block_equals (handle, other_a.handle);
}

void nano::receive_block::serialize (nano::stream & stream_a) const
{
	if (rsnano::rsn_receive_block_serialize (handle, &stream_a) < 0)
		throw std::runtime_error ("could not serialize receive_block");
}

void nano::receive_block::serialize_json (std::string & string_a, bool single_line) const
{
	boost::property_tree::ptree tree;
	serialize_json (tree);
	std::stringstream ostream;
	boost::property_tree::write_json (ostream, tree, !single_line);
	string_a = ostream.str ();
}

void nano::receive_block::serialize_json (boost::property_tree::ptree & tree) const
{
	if (rsnano::rsn_receive_block_serialize_json (handle, &tree) < 0)
		throw std::runtime_error ("could not JSON serialize receive_block");
}

nano::receive_block::receive_block ()
{
	rsnano::ReceiveBlockDto dto;
	dto.work = 0;
	handle = rsnano::rsn_receive_block_create (&dto);
}

nano::receive_block::receive_block (nano::block_hash const & previous_a, nano::block_hash const & source_a, nano::raw_key const & prv_a, nano::public_key const & pub_a, uint64_t work_a)
{
	debug_assert (pub_a != nullptr);
	rsnano::ReceiveBlockDto2 dto;
	std::copy (std::begin (previous_a.bytes), std::end (previous_a.bytes), std::begin (dto.previous));
	std::copy (std::begin (source_a.bytes), std::end (source_a.bytes), std::begin (dto.source));
	std::copy (std::begin (prv_a.bytes), std::end (prv_a.bytes), std::begin (dto.priv_key));
	std::copy (std::begin (pub_a.bytes), std::end (pub_a.bytes), std::begin (dto.pub_key));
	dto.work = work_a;
	handle = rsnano::rsn_receive_block_create2 (&dto);
	if (handle == nullptr)
		throw std::runtime_error ("could not create receive_block");
}

nano::receive_block::receive_block (bool & error_a, nano::stream & stream_a)
{
	handle = rsnano::rsn_receive_block_deserialize (&stream_a);
	error_a = handle == nullptr;
}

nano::receive_block::receive_block (bool & error_a, boost::property_tree::ptree const & tree_a)
{
	handle = rsnano::rsn_receive_block_deserialize_json (&tree_a);
	error_a = handle == nullptr;
}

nano::receive_block::receive_block (const nano::receive_block & other)
{
	cached_hash = other.cached_hash;
	if (other.handle == nullptr)
	{
		handle = nullptr;
	}
	else
	{
		handle = rsnano::rsn_receive_block_clone (other.handle);
	}
}

nano::receive_block::receive_block (nano::receive_block && other)
{
	cached_hash = other.cached_hash;
	handle = other.handle;
	other.handle = nullptr;
}

nano::receive_block::receive_block (rsnano::ReceiveBlockHandle * handle_a) :
	handle (handle_a)
{
}

nano::receive_block::~receive_block ()
{
	if (handle != nullptr)
		rsnano::rsn_receive_block_destroy (handle);
}

uint64_t nano::receive_block::block_work () const
{
	return rsnano::rsn_receive_block_work (handle);
}

void nano::receive_block::block_work_set (uint64_t work_a)
{
	rsnano::rsn_receive_block_work_set (handle, work_a);
}

bool nano::receive_block::operator== (nano::block const & other_a) const
{
	return blocks_equal (*this, other_a);
}

bool nano::receive_block::valid_predecessor (nano::block const & block_a) const
{
	bool result;
	switch (block_a.type ())
	{
		case nano::block_type::send:
		case nano::block_type::receive:
		case nano::block_type::open:
		case nano::block_type::change:
			result = true;
			break;
		default:
			result = false;
			break;
	}
	return result;
}

nano::block_hash nano::receive_block::previous () const
{
	uint8_t buffer[32];
	rsnano::rsn_receive_block_previous (handle, &buffer);
	nano::block_hash result;
	std::copy (std::begin (buffer), std::end (buffer), std::begin (result.bytes));
	return result;
}

void nano::receive_block::previous_set (nano::block_hash previous_a)
{
	uint8_t buffer[32];
	std::copy (std::begin (previous_a.bytes), std::end (previous_a.bytes), std::begin (buffer));
	rsnano::rsn_receive_block_previous_set (handle, &buffer);
}

nano::block_hash nano::receive_block::source () const
{
	uint8_t buffer[32];
	rsnano::rsn_receive_block_source (handle, &buffer);
	nano::block_hash result;
	std::copy (std::begin (buffer), std::end (buffer), std::begin (result.bytes));
	return result;
}

void nano::receive_block::source_set (nano::block_hash source_a)
{
	uint8_t buffer[32];
	std::copy (std::begin (source_a.bytes), std::end (source_a.bytes), std::begin (buffer));
	rsnano::rsn_receive_block_source_set (handle, &buffer);
}

nano::root nano::receive_block::root () const
{
	return previous ();
}

nano::signature nano::receive_block::block_signature () const
{
	uint8_t buffer[64];
	rsnano::rsn_receive_block_signature (handle, &buffer);
	nano::signature result;
	std::copy (std::begin (buffer), std::end (buffer), std::begin (result.bytes));
	return result;
}

void nano::receive_block::signature_set (nano::signature const & signature_a)
{
	uint8_t buffer[64];
	std::copy (std::begin (signature_a.bytes), std::end (signature_a.bytes), std::begin (buffer));
	rsnano::rsn_receive_block_signature_set (handle, &buffer);
}

nano::block_type nano::receive_block::type () const
{
	return nano::block_type::receive;
}

void nano::receive_block::sign_zero ()
{
	nano::signature sig{ 0 };
	signature_set (sig);
}

void nano::receive_block::zero ()
{
	block_work_set (0);
	sign_zero ();
	previous_set (0);
	source_set (0);
}

std::size_t nano::receive_block::size ()
{
	return rsnano::rsn_receive_block_size ();
}

rsnano::BlockDto nano::receive_block::to_block_dto () const
{
	rsnano::BlockDto dto;
	dto.block_type = static_cast<uint8_t> (nano::block_type::receive);
	dto.handle = handle;
	return dto;
}

nano::block_hash nano::receive_block::generate_hash () const
{
	uint8_t bytes[32];
	rsnano::rsn_receive_block_hash (handle, &bytes);
	nano::block_hash result;
	std::copy (std::begin (bytes), std::end (bytes), std::begin (result.bytes));
	return result;
}

void * nano::receive_block::get_handle () const
{
	return handle;
}

nano::block_details::block_details ()
{
	auto result = rsnano::rsn_block_details_create (static_cast<uint8_t> (nano::epoch::epoch_0), false, false, false, &dto);
	if (result < 0)
	{
		throw std::runtime_error ("could not create block details");
	}
}

nano::block_details::block_details (nano::epoch const epoch_a, bool const is_send_a, bool const is_receive_a, bool const is_epoch_a)
{
	auto result = rsnano::rsn_block_details_create (static_cast<uint8_t> (epoch_a), is_send_a, is_receive_a, is_epoch_a, &dto);
	if (result < 0)
	{
		throw std::runtime_error ("could not create block details");
	}
}

nano::block_details::block_details (rsnano::BlockDetailsDto dto_a)
{
	dto = dto_a;
}

bool nano::block_details::operator== (nano::block_details const & other_a) const
{
	return dto.epoch == other_a.dto.epoch && dto.is_send == other_a.dto.is_send && dto.is_receive == other_a.dto.is_receive && dto.is_epoch == other_a.dto.is_epoch;
}

nano::epoch nano::block_details::epoch () const
{
	return static_cast<nano::epoch> (dto.epoch);
}

bool nano::block_details::is_send () const
{
	return dto.is_send;
}

bool nano::block_details::is_receive () const
{
	return dto.is_receive;
}

bool nano::block_details::is_epoch () const
{
	return dto.is_epoch;
}

void nano::block_details::serialize (nano::stream & stream_a) const
{
	auto result = rsnano::rsn_block_details_serialize (&dto, &stream_a);
	if (result < 0)
	{
		throw new std::runtime_error ("could not serialize block details");
	}
}

bool nano::block_details::deserialize (nano::stream & stream_a)
{
	auto result = rsnano::rsn_block_details_deserialize (&dto, &stream_a);
	return result != 0;
}

std::string nano::state_subtype (nano::block_details const details_a)
{
	debug_assert (details_a.is_epoch () + details_a.is_receive () + details_a.is_send () <= 1);
	if (details_a.is_send ())
	{
		return "send";
	}
	else if (details_a.is_receive ())
	{
		return "receive";
	}
	else if (details_a.is_epoch ())
	{
		return "epoch";
	}
	else
	{
		return "change";
	}
}

nano::block_sideband::block_sideband ()
{
	dto.source_epoch = static_cast<uint8_t> (epoch::epoch_0);
	dto.height = 0;
	dto.timestamp = 0;
	rsnano::rsn_block_details_create (static_cast<uint8_t> (epoch::epoch_0), false, false, false, &dto.details);
	std::fill (std::begin (dto.successor), std::end (dto.successor), 0);
	std::fill (std::begin (dto.account), std::end (dto.account), 0);
	std::fill (std::begin (dto.balance), std::end (dto.balance), 0);
}

nano::block_sideband::block_sideband (rsnano::BlockSidebandDto const & dto_a) :
	dto (dto_a)
{
}

nano::block_sideband::block_sideband (nano::account const & account_a, nano::block_hash const & successor_a, nano::amount const & balance_a, uint64_t const height_a, uint64_t const timestamp_a, nano::block_details const & details_a, nano::epoch const source_epoch_a)
{
	dto.source_epoch = static_cast<uint8_t> (source_epoch_a);
	dto.height = height_a;
	dto.timestamp = timestamp_a;
	dto.details = details_a.dto;
	std::copy (std::begin (successor_a.bytes), std::end (successor_a.bytes), std::begin (dto.successor));
	std::copy (std::begin (account_a.bytes), std::end (account_a.bytes), std::begin (dto.account));
	std::copy (std::begin (balance_a.bytes), std::end (balance_a.bytes), std::begin (dto.balance));
}

nano::block_sideband::block_sideband (nano::account const & account_a, nano::block_hash const & successor_a, nano::amount const & balance_a, uint64_t const height_a, uint64_t const timestamp_a, nano::epoch const epoch_a, bool const is_send, bool const is_receive, bool const is_epoch, nano::epoch const source_epoch_a)
{
	dto.source_epoch = static_cast<uint8_t> (source_epoch_a);
	dto.height = height_a;
	dto.timestamp = timestamp_a;
	rsnano::rsn_block_details_create (static_cast<uint8_t> (epoch_a), is_send, is_receive, is_epoch, &dto.details);
	std::copy (std::begin (successor_a.bytes), std::end (successor_a.bytes), std::begin (dto.successor));
	std::copy (std::begin (account_a.bytes), std::end (account_a.bytes), std::begin (dto.account));
	std::copy (std::begin (balance_a.bytes), std::end (balance_a.bytes), std::begin (dto.balance));
}

rsnano::BlockSidebandDto const & nano::block_sideband::as_dto () const
{
	return dto;
}

size_t nano::block_sideband::size (nano::block_type type_a)
{
	int32_t res{ 0 };
	size_t result{ rsnano::rsn_block_sideband_size (static_cast<uint8_t> (type_a), &res) };
	if (res != 0)
		throw std::runtime_error ("rsn_block_sideband_size failed");

	return result;
}

void nano::block_sideband::serialize (nano::stream & stream_a, nano::block_type type_a) const
{
	auto result{ rsnano::rsn_block_sideband_serialize (&dto, &stream_a, static_cast<uint8_t> (type_a)) };
	if (result != 0)
	{
		throw std::runtime_error ("block_sideband serialization failed");
	}
}

bool nano::block_sideband::deserialize (nano::stream & stream_a, nano::block_type type_a)
{
	return rsnano::rsn_block_sideband_deserialize (&dto, &stream_a, static_cast<uint8_t> (type_a)) != 0;
}

nano::epoch nano::block_sideband::source_epoch () const
{
	return static_cast<nano::epoch> (dto.source_epoch);
}

void nano::block_sideband::set_source_epoch (nano::epoch epoch)
{
	dto.source_epoch = static_cast<uint8_t> (epoch);
}

uint64_t nano::block_sideband::height () const
{
	return dto.height;
}

void nano::block_sideband::set_height (uint64_t h)
{
	dto.height = h;
}

uint64_t nano::block_sideband::timestamp () const
{
	return dto.timestamp;
}

void nano::block_sideband::set_timestamp (uint64_t ts)
{
	dto.timestamp = ts;
}

nano::block_details nano::block_sideband::details () const
{
	return nano::block_details (dto.details);
}

nano::block_hash nano::block_sideband::successor () const
{
	nano::block_hash result;
	std::copy (std::begin (dto.successor), std::end (dto.successor), std::begin (result.bytes));
	return result;
}

void nano::block_sideband::set_successor (nano::block_hash successor_a)
{
	std::copy (std::begin (successor_a.bytes), std::end (successor_a.bytes), std::begin (dto.successor));
}

nano::account nano::block_sideband::account () const
{
	nano::account result;
	std::copy (std::begin (dto.account), std::end (dto.account), std::begin (result.bytes));
	return result;
}

nano::amount nano::block_sideband::balance () const
{
	nano::amount result;
	std::copy (std::begin (dto.balance), std::end (dto.balance), std::begin (result.bytes));
	return result;
}

std::shared_ptr<nano::block> nano::block_uniquer::unique (std::shared_ptr<nano::block> const & block_a)
{
	auto result (block_a);
	if (result != nullptr)
	{
		nano::uint256_union key (block_a->full_hash ());
		nano::lock_guard<nano::mutex> lock (mutex);
		auto & existing (blocks[key]);
		if (auto block_l = existing.lock ())
		{
			result = block_l;
		}
		else
		{
			existing = block_a;
		}
		release_assert (std::numeric_limits<CryptoPP::word32>::max () > blocks.size ());
		for (auto i (0); i < cleanup_count && !blocks.empty (); ++i)
		{
			auto random_offset (nano::random_pool::generate_word32 (0, static_cast<CryptoPP::word32> (blocks.size () - 1)));
			auto existing (std::next (blocks.begin (), random_offset));
			if (existing == blocks.end ())
			{
				existing = blocks.begin ();
			}
			if (existing != blocks.end ())
			{
				if (auto block_l = existing->second.lock ())
				{
					// Still live
				}
				else
				{
					blocks.erase (existing);
				}
			}
		}
	}
	return result;
}

size_t nano::block_uniquer::size ()
{
	nano::lock_guard<nano::mutex> lock (mutex);
	return blocks.size ();
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (block_uniquer & block_uniquer, std::string const & name)
{
	auto count = block_uniquer.size ();
	auto sizeof_element = sizeof (block_uniquer::value_type);
	auto composite = std::make_unique<container_info_composite> (name);
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "blocks", count, sizeof_element }));
	return composite;
}
