#include <nano/lib/blocks.hpp>
#include <nano/lib/memory.hpp>
#include <nano/lib/rsnanoutils.hpp>
#include <nano/lib/work.hpp>
#include <nano/node/active_transactions.hpp>
#include <nano/node/common.hpp>
#include <nano/node/election.hpp>
#include <nano/node/network.hpp>
#include <nano/node/wallet.hpp>
#include <nano/secure/buffer.hpp>

#include <boost/endian/conversion.hpp>
#include <boost/format.hpp>
#include <boost/pool/pool_alloc.hpp>
#include <boost/variant/get.hpp>

#include <numeric>
#include <sstream>

std::bitset<16> constexpr nano::message_header::telemetry_size_mask;

std::chrono::seconds constexpr nano::telemetry_cache_cutoffs::dev;
std::chrono::seconds constexpr nano::telemetry_cache_cutoffs::beta;
std::chrono::seconds constexpr nano::telemetry_cache_cutoffs::live;

uint64_t nano::ip_address_hash_raw (boost::asio::ip::address const & ip_a, uint16_t port)
{
	debug_assert (ip_a.is_v6 ());
	uint64_t result;
	nano::uint128_union address;
	address.bytes = ip_a.to_v6 ().to_bytes ();
	blake2b_state state;
	blake2b_init (&state, sizeof (result));
	blake2b_update (&state, nano::hardened_constants::get ().random_128.bytes.data (), nano::hardened_constants::get ().random_128.bytes.size ());
	if (port != 0)
	{
		blake2b_update (&state, &port, sizeof (port));
	}
	blake2b_update (&state, address.bytes.data (), address.bytes.size ());
	blake2b_final (&state, &result, sizeof (result));
	return result;
}

nano::message_header::message_header (nano::network_constants const & constants, nano::message_type type_a)
{
	auto constants_dto{ constants.to_dto () };
	handle = rsnano::rsn_message_header_create (&constants_dto, static_cast<uint8_t> (type_a), -1);
}

nano::message_header::message_header (nano::network_constants const & constants, nano::message_type type_a, uint8_t version_using_a)
{
	auto constants_dto{ constants.to_dto () };
	handle = rsnano::rsn_message_header_create (&constants_dto, static_cast<uint8_t> (type_a), version_using_a);
}

nano::message_header::message_header (nano::message_header const & other_a) :
	handle{ rsnano::rsn_message_header_clone (other_a.handle) }
{
}

nano::message_header::message_header (nano::message_header && other_a) :
	handle{ other_a.handle }
{
	other_a.handle = nullptr;
}

nano::message_header::message_header (rsnano::MessageHeaderHandle * handle_a) :
	handle{ handle_a }
{
}

nano::message_header::message_header (bool & error_a, nano::stream & stream_a) :
	handle{ rsnano::rsn_message_header_empty () }
{
	if (!error_a)
	{
		error_a = deserialize (stream_a);
	}
}

nano::message_header & nano::message_header::operator= (nano::message_header && other_a)
{
	handle = other_a.handle;
	other_a.handle = nullptr;
	return *this;
}

nano::message_header & nano::message_header::operator= (message_header const & other_a)
{
	handle = rsnano::rsn_message_header_clone (other_a.handle);
	return *this;
}

nano::message_header::~message_header ()
{
	if (handle != nullptr)
		rsnano::rsn_message_header_destroy (handle);
}

void nano::message_header::serialize (nano::stream & stream_a) const
{
	if (!rsnano::rsn_message_header_serialize (handle, &stream_a))
	{
		throw new std::runtime_error ("could not serialize message header");
	}
}

bool nano::message_header::deserialize (nano::stream & stream_a)
{
	auto error = !rsnano::rsn_message_header_deserialize (handle, &stream_a);
	return error;
}

std::string nano::message_type_to_string (nano::message_type message_type_l)
{
	rsnano::StringDto result;
	rsnano::rsn_message_type_to_string (static_cast<uint8_t> (message_type_l), &result);
	return rsnano::convert_dto_to_string (result);
}

std::string nano::message_header::to_string ()
{
	rsnano::StringDto result;
	rsnano::rsn_message_header_to_string (handle, &result);
	return rsnano::convert_dto_to_string (result);
}

nano::message::message (rsnano::MessageHandle * handle_a) :
	handle (handle_a)
{
}

nano::message::~message ()
{
	rsnano::rsn_message_destroy (handle);
}

std::shared_ptr<std::vector<uint8_t>> nano::message::to_bytes () const
{
	auto bytes = std::make_shared<std::vector<uint8_t>> ();
	nano::vectorstream stream (*bytes);
	serialize (stream);
	return bytes;
}

nano::shared_const_buffer nano::message::to_shared_const_buffer () const
{
	return shared_const_buffer (to_bytes ());
}

nano::message_header nano::message::get_header () const
{
	return nano::message_header{ rsnano::rsn_message_header (handle) };
}

void nano::message::set_header (nano::message_header const & header_a)
{
	rsnano::rsn_message_set_header (handle, header_a.handle);
}

nano::block_type nano::message_header::block_type () const
{
	return static_cast<nano::block_type> (rsnano::rsn_message_header_block_type (handle));
}

void nano::message_header::block_type_set (nano::block_type type_a)
{
	rsnano::rsn_message_header_set_block_type (handle, static_cast<uint8_t> (type_a));
}

uint8_t nano::message_header::count_get () const
{
	return rsnano::rsn_message_header_count (handle);
}

void nano::message_header::count_set (uint8_t count_a)
{
	rsnano::rsn_message_header_set_count (handle, count_a);
}

void nano::message_header::flag_set (uint8_t flag_a)
{
	// Flags from 8 are block_type & count
	debug_assert (flag_a < 8);
	set_extension (flag_a, true);
}

bool nano::message_header::bulk_pull_is_count_present () const
{
	return rsnano::rsn_message_header_bulk_pull_is_count_present (handle);
}

bool nano::message_header::node_id_handshake_is_query () const
{
	auto result (false);
	if (get_type () == nano::message_type::node_id_handshake)
	{
		if (test_extension (node_id_handshake_query_flag))
		{
			result = true;
		}
	}
	return result;
}

bool nano::message_header::node_id_handshake_is_response () const
{
	auto result (false);
	if (get_type () == nano::message_type::node_id_handshake)
	{
		if (test_extension (node_id_handshake_response_flag))
		{
			result = true;
		}
	}
	return result;
}

std::size_t nano::message_header::payload_length_bytes () const
{
	switch (get_type ())
	{
		case nano::message_type::bulk_pull:
		{
			return nano::bulk_pull::size () + (bulk_pull_is_count_present () ? nano::bulk_pull::extended_parameters_size : 0);
		}
		case nano::message_type::bulk_push:
		case nano::message_type::telemetry_req:
		{
			// These don't have a payload
			return 0;
		}
		case nano::message_type::frontier_req:
		{
			return nano::frontier_req::size ();
		}
		case nano::message_type::bulk_pull_account:
		{
			return nano::bulk_pull_account::size ();
		}
		case nano::message_type::keepalive:
		{
			return nano::keepalive::size ();
		}
		case nano::message_type::publish:
		{
			return nano::block::size (block_type ());
		}
		case nano::message_type::confirm_ack:
		{
			return nano::confirm_ack::size (count_get ());
		}
		case nano::message_type::confirm_req:
		{
			return nano::confirm_req::size (block_type (), count_get ());
		}
		case nano::message_type::node_id_handshake:
		{
			return nano::node_id_handshake::size (*this);
		}
		case nano::message_type::telemetry_ack:
		{
			return nano::telemetry_ack::size (*this);
		}
		default:
		{
			debug_assert (false);
			return 0;
		}
	}
}

nano::networks nano::message_header::get_network () const
{
	return static_cast<nano::networks> (rsnano::rsn_message_header_network (handle));
}

uint8_t nano::message_header::get_version_max () const
{
	return rsnano::rsn_message_header_version_max (handle);
}

uint8_t nano::message_header::get_version_using () const
{
	return rsnano::rsn_message_header_version_using (handle);
}

uint8_t nano::message_header::get_version_min () const
{
	return rsnano::rsn_message_header_version_min (handle);
}

nano::message_type nano::message_header::get_type () const
{
	return static_cast<nano::message_type> (rsnano::rsn_message_header_type (handle));
}

uint16_t nano::message_header::get_extensions_raw () const
{
	return rsnano::rsn_message_header_extensions (handle);
}

std::bitset<16> nano::message_header::get_extensions () const
{
	return std::bitset<16> (static_cast<unsigned long long> (get_extensions_raw ()));
}

void nano::message_header::set_extensions (std::bitset<16> const & bits)
{
	auto value = static_cast<uint16_t> (bits.to_ulong ());
	rsnano::rsn_message_header_set_extensions (handle, value);
}

bool nano::message_header::test_extension (std::size_t position) const
{
	return rsnano::rsn_message_header_test_extension (handle, position);
}

void nano::message_header::set_extension (std::size_t position, bool value)
{
	rsnano::rsn_message_header_set_extension (handle, position, value);
}

size_t nano::message_header::size ()
{
	return rsnano::rsn_message_header_size ();
}

// MTU - IP header - UDP header
std::size_t const nano::message_parser::max_safe_udp_message_size = 508;

std::string nano::message_parser::status_string ()
{
	switch (status)
	{
		case nano::message_parser::parse_status::success:
		{
			return "success";
		}
		case nano::message_parser::parse_status::insufficient_work:
		{
			return "insufficient_work";
		}
		case nano::message_parser::parse_status::invalid_header:
		{
			return "invalid_header";
		}
		case nano::message_parser::parse_status::invalid_message_type:
		{
			return "invalid_message_type";
		}
		case nano::message_parser::parse_status::invalid_keepalive_message:
		{
			return "invalid_keepalive_message";
		}
		case nano::message_parser::parse_status::invalid_publish_message:
		{
			return "invalid_publish_message";
		}
		case nano::message_parser::parse_status::invalid_confirm_req_message:
		{
			return "invalid_confirm_req_message";
		}
		case nano::message_parser::parse_status::invalid_confirm_ack_message:
		{
			return "invalid_confirm_ack_message";
		}
		case nano::message_parser::parse_status::invalid_node_id_handshake_message:
		{
			return "invalid_node_id_handshake_message";
		}
		case nano::message_parser::parse_status::invalid_telemetry_req_message:
		{
			return "invalid_telemetry_req_message";
		}
		case nano::message_parser::parse_status::invalid_telemetry_ack_message:
		{
			return "invalid_telemetry_ack_message";
		}
		case nano::message_parser::parse_status::outdated_version:
		{
			return "outdated_version";
		}
		case nano::message_parser::parse_status::duplicate_publish_message:
		{
			return "duplicate_publish_message";
		}
	}

	debug_assert (false);

	return "[unknown parse_status]";
}

nano::message_parser::message_parser (nano::network_filter & publish_filter_a, nano::block_uniquer & block_uniquer_a, nano::vote_uniquer & vote_uniquer_a, nano::message_visitor & visitor_a, nano::work_pool & pool_a, nano::network_constants const & network) :
	publish_filter (publish_filter_a),
	block_uniquer (block_uniquer_a),
	vote_uniquer (vote_uniquer_a),
	visitor (visitor_a),
	pool (pool_a),
	status (parse_status::success),
	network{ network }
{
}

void nano::message_parser::deserialize_buffer (uint8_t const * buffer_a, std::size_t size_a)
{
	status = parse_status::success;
	auto error (false);
	if (size_a <= max_safe_udp_message_size)
	{
		// Guaranteed to be deliverable
		nano::bufferstream stream (buffer_a, size_a);
		nano::message_header header (error, stream);
		if (!error)
		{
			if (header.get_version_using () < network.protocol_version_min)
			{
				status = parse_status::outdated_version;
			}
			else
			{
				switch (header.get_type ())
				{
					case nano::message_type::keepalive:
					{
						deserialize_keepalive (stream, header);
						break;
					}
					case nano::message_type::publish:
					{
						nano::uint128_t digest;
						if (!publish_filter.apply (buffer_a + message_header::size (), size_a - message_header::size (), &digest))
						{
							deserialize_publish (stream, header, digest);
						}
						else
						{
							status = parse_status::duplicate_publish_message;
						}
						break;
					}
					case nano::message_type::confirm_req:
					{
						deserialize_confirm_req (stream, header);
						break;
					}
					case nano::message_type::confirm_ack:
					{
						deserialize_confirm_ack (stream, header);
						break;
					}
					case nano::message_type::node_id_handshake:
					{
						deserialize_node_id_handshake (stream, header);
						break;
					}
					case nano::message_type::telemetry_req:
					{
						deserialize_telemetry_req (stream, header);
						break;
					}
					case nano::message_type::telemetry_ack:
					{
						deserialize_telemetry_ack (stream, header);
						break;
					}
					default:
					{
						status = parse_status::invalid_message_type;
						break;
					}
				}
			}
		}
		else
		{
			status = parse_status::invalid_header;
		}
	}
}

void nano::message_parser::deserialize_keepalive (nano::stream & stream_a, nano::message_header const & header_a)
{
	auto error (false);
	nano::keepalive incoming (error, stream_a, header_a);
	if (!error && at_end (stream_a))
	{
		visitor.keepalive (incoming);
	}
	else
	{
		status = parse_status::invalid_keepalive_message;
	}
}

void nano::message_parser::deserialize_publish (nano::stream & stream_a, nano::message_header const & header_a, nano::uint128_t const & digest_a)
{
	auto error (false);
	nano::publish incoming (error, stream_a, header_a, digest_a, &block_uniquer);
	if (!error && at_end (stream_a))
	{
		auto block{ incoming.get_block () };
		if (!network.work.validate_entry (*block))
		{
			visitor.publish (incoming);
		}
		else
		{
			status = parse_status::insufficient_work;
		}
	}
	else
	{
		status = parse_status::invalid_publish_message;
	}
}

void nano::message_parser::deserialize_confirm_req (nano::stream & stream_a, nano::message_header const & header_a)
{
	auto error (false);
	nano::confirm_req incoming (error, stream_a, header_a, &block_uniquer);
	if (!error && at_end (stream_a))
	{
		if (incoming.get_block () == nullptr || !network.work.validate_entry (*incoming.get_block ()))
		{
			visitor.confirm_req (incoming);
		}
		else
		{
			status = parse_status::insufficient_work;
		}
	}
	else
	{
		status = parse_status::invalid_confirm_req_message;
	}
}

void nano::message_parser::deserialize_confirm_ack (nano::stream & stream_a, nano::message_header const & header_a)
{
	auto error (false);
	nano::confirm_ack incoming (error, stream_a, header_a, &vote_uniquer);
	if (!error && at_end (stream_a))
	{
		visitor.confirm_ack (incoming);
	}
	else
	{
		status = parse_status::invalid_confirm_ack_message;
	}
}

void nano::message_parser::deserialize_node_id_handshake (nano::stream & stream_a, nano::message_header const & header_a)
{
	bool error_l (false);
	nano::node_id_handshake incoming (error_l, stream_a, header_a);
	if (!error_l && at_end (stream_a))
	{
		visitor.node_id_handshake (incoming);
	}
	else
	{
		status = parse_status::invalid_node_id_handshake_message;
	}
}

void nano::message_parser::deserialize_telemetry_req (nano::stream & stream_a, nano::message_header const & header_a)
{
	nano::telemetry_req incoming (header_a);
	if (at_end (stream_a))
	{
		visitor.telemetry_req (incoming);
	}
	else
	{
		status = parse_status::invalid_telemetry_req_message;
	}
}

void nano::message_parser::deserialize_telemetry_ack (nano::stream & stream_a, nano::message_header const & header_a)
{
	bool error_l (false);
	nano::telemetry_ack incoming (error_l, stream_a, header_a);
	// Intentionally not checking if at the end of stream, because these messages support backwards/forwards compatibility
	if (!error_l)
	{
		visitor.telemetry_ack (incoming);
	}
	else
	{
		status = parse_status::invalid_telemetry_ack_message;
	}
}

bool nano::message_parser::at_end (nano::stream & stream_a)
{
	uint8_t junk;
	auto end (nano::try_read (stream_a, junk));
	return end;
}

rsnano::MessageHandle * create_keepalive_handle (nano::network_constants const & constants, int16_t version_using)
{
	auto constants_dto{ constants.to_dto () };
	return rsnano::rsn_message_keepalive_create (&constants_dto, version_using);
}

nano::keepalive::keepalive (nano::network_constants const & constants) :
	message (create_keepalive_handle (constants, -1))
{
}

nano::keepalive::keepalive (nano::network_constants const & constants, uint8_t version_using_a) :
	message (create_keepalive_handle (constants, version_using_a))
{
}

nano::keepalive::keepalive (bool & error_a, nano::stream & stream_a, nano::message_header const & header_a) :
	message (rsnano::rsn_message_keepalive_create2 (header_a.handle))
{
	if (!error_a)
	{
		error_a = deserialize (stream_a);
	}
}

nano::keepalive::keepalive (keepalive const & other_a) :
	message (rsnano::rsn_message_keepalive_clone (other_a.handle))
{
}

void nano::keepalive::visit (nano::message_visitor & visitor_a) const
{
	visitor_a.keepalive (*this);
}

void nano::keepalive::serialize (nano::stream & stream_a) const
{
	if (!rsnano::rsn_message_keepalive_serialize (handle, &stream_a))
	{
		throw std::runtime_error ("could not serialize keepalive");
	}
}

bool nano::keepalive::deserialize (nano::stream & stream_a)
{
	bool error = !rsnano::rsn_message_keepalive_deserialize (handle, &stream_a);
	return error;
}

bool nano::keepalive::operator== (nano::keepalive const & other_a) const
{
	return get_peers () == other_a.get_peers ();
}

std::array<nano::endpoint, 8> nano::keepalive::get_peers () const
{
	rsnano::EndpointDto dtos[8];
	rsnano::rsn_message_keepalive_peers (handle, &dtos[0]);
	std::array<nano::endpoint, 8> result;
	for (auto i = 0; i < 8; ++i)
	{
		result[i] = rsnano::dto_to_udp_endpoint (dtos[i]);
	}
	return result;
}

void nano::keepalive::set_peers (std::array<nano::endpoint, 8> const & peers_a)
{
	rsnano::EndpointDto dtos[8];
	for (auto i = 0; i < 8; ++i)
	{
		dtos[i] = rsnano::udp_endpoint_to_dto (peers_a[i]);
	}
	rsnano::rsn_message_keepalive_set_peers (handle, dtos);
}

std::size_t nano::keepalive::size ()
{
	return rsnano::rsn_message_keepalive_size ();
}

rsnano::MessageHandle * create_publish_handle2 (nano::message_header const & header_a, nano::uint128_t const & digest_a)
{
	std::vector<uint8_t> bytes;
	boost::multiprecision::export_bits (digest_a, std::back_inserter (bytes), 8);
	return rsnano::rsn_message_publish_create2 (header_a.handle, bytes.data ());
}

nano::publish::publish (bool & error_a, nano::stream & stream_a, nano::message_header const & header_a, nano::uint128_t const & digest_a, nano::block_uniquer * uniquer_a) :
	message (create_publish_handle2 (header_a, digest_a))
{
	if (!error_a)
	{
		error_a = deserialize (stream_a, uniquer_a);
	}
}

rsnano::MessageHandle * create_publish_handle (nano::network_constants const & constants, std::shared_ptr<nano::block> const & block_a)
{
	auto constants_dto{ constants.to_dto () };
	return rsnano::rsn_message_publish_create (&constants_dto, block_a->get_handle ());
}

nano::publish::publish (nano::network_constants const & constants, std::shared_ptr<nano::block> const & block_a) :
	message (create_publish_handle (constants, block_a))
{
}

nano::publish::publish (nano::publish const & other_a) :
	message (rsnano::rsn_message_publish_clone (other_a.handle))
{
}

void nano::publish::serialize (nano::stream & stream_a) const
{
	if (!rsnano::rsn_message_publish_serialize (handle, &stream_a))
	{
		throw std::runtime_error ("could not serialize publish message");
	}
}

bool nano::publish::deserialize (nano::stream & stream_a, nano::block_uniquer * uniquer_a)
{
	rsnano::BlockUniquerHandle * uniquer_handle = nullptr;
	if (uniquer_a != nullptr)
	{
		uniquer_handle = uniquer_a->handle;
	}
	bool error = !rsnano::rsn_message_publish_deserialize (handle, &stream_a, uniquer_handle);
	return error;
}

void nano::publish::visit (nano::message_visitor & visitor_a) const
{
	visitor_a.publish (*this);
}

bool nano::publish::operator== (nano::publish const & other_a) const
{
	return *get_block () == *other_a.get_block ();
}

std::shared_ptr<nano::block> nano::publish::get_block () const
{
	auto block_handle = rsnano::rsn_message_publish_block (handle);
	if (block_handle == nullptr)
		return nullptr;
	return nano::block_handle_to_block (block_handle);
}

nano::uint128_t nano::publish::get_digest () const
{
	std::uint8_t bytes[16];
	rsnano::rsn_message_publish_digest (handle, &bytes[0]);
	nano::uint128_t result;
	boost::multiprecision::import_bits (result, std::begin (bytes), std::end (bytes));
	return result;
}

void nano::publish::set_digest (nano::uint128_t digest_a)
{
	std::uint8_t bytes[16];
	boost::multiprecision::export_bits (digest_a, std::begin (bytes), 8);
	rsnano::rsn_message_publish_set_digest (handle, &bytes[0]);
}

rsnano::MessageHandle * create_confirm_req_handle (nano::network_constants const & constants, nano::block const * block_a, std::vector<std::pair<nano::block_hash, nano::root>> roots_hashes_a)
{
	auto constants_dto{ constants.to_dto () };
	rsnano::BlockHandle * block_handle = nullptr;
	if (block_a != nullptr)
	{
		block_handle = block_a->get_handle ();
	}

	size_t hashes_count = roots_hashes_a.size ();
	std::vector<rsnano::HashRootPair> dtos;
	dtos.reserve (hashes_count);
	for (const auto & i : roots_hashes_a)
	{
		rsnano::HashRootPair dto;
		std::copy (std::begin (i.first.bytes), std::end (i.first.bytes), std::begin (dto.block_hash));
		std::copy (std::begin (i.second.bytes), std::end (i.second.bytes), std::begin (dto.root));
		dtos.push_back (dto);
	}

	return rsnano::rsn_message_confirm_req_create (&constants_dto, block_handle, dtos.data (), hashes_count);
}

nano::confirm_req::confirm_req (bool & error_a, nano::stream & stream_a, nano::message_header const & header_a, nano::block_uniquer * uniquer_a) :
	message (rsnano::rsn_message_confirm_req_create2 (header_a.handle))
{
	if (!error_a)
	{
		error_a = deserialize (stream_a, uniquer_a);
	}
}

nano::confirm_req::confirm_req (nano::network_constants const & constants, std::shared_ptr<nano::block> const & block_a) :
	message (create_confirm_req_handle (constants, block_a.get (), std::vector<std::pair<nano::block_hash, nano::root>> ()))
{
}

nano::confirm_req::confirm_req (nano::network_constants const & constants, std::vector<std::pair<nano::block_hash, nano::root>> const & roots_hashes_a) :
	message (create_confirm_req_handle (constants, nullptr, roots_hashes_a))
{
}

nano::confirm_req::confirm_req (nano::network_constants const & constants, nano::block_hash const & hash_a, nano::root const & root_a) :
	message (create_confirm_req_handle (constants, nullptr, std::vector<std::pair<nano::block_hash, nano::root>> (1, std::make_pair (hash_a, root_a))))
{
}

nano::confirm_req::confirm_req (nano::confirm_req const & other_a) :
	message (rsnano::rsn_message_confirm_req_clone (other_a.handle))
{
}

std::shared_ptr<nano::block> nano::confirm_req::get_block () const
{
	auto block_handle = rsnano::rsn_message_confirm_req_block (handle);
	std::shared_ptr<nano::block> result;
	if (block_handle != nullptr)
	{
		result = nano::block_handle_to_block (block_handle);
	}
	return result;
}

std::vector<std::pair<nano::block_hash, nano::root>> nano::confirm_req::get_roots_hashes () const
{
	auto count = rsnano::rsn_message_confirm_req_roots_hashes_count (handle);
	std::vector<rsnano::HashRootPair> dtos;
	dtos.resize (count);
	rsnano::rsn_message_confirm_req_roots_hashes (handle, dtos.data ());
	std::vector<std::pair<nano::block_hash, nano::root>> result;
	result.reserve (dtos.size ());
	for (const auto & i : dtos)
	{
		nano::block_hash hash;
		nano::root root;
		std::copy (std::begin (i.block_hash), std::end (i.block_hash), std::begin (hash.bytes));
		std::copy (std::begin (i.root), std::end (i.root), std::begin (root.bytes));
		result.emplace_back (hash, root);
	}
	return result;
}

void nano::confirm_req::visit (nano::message_visitor & visitor_a) const
{
	visitor_a.confirm_req (*this);
}

void nano::confirm_req::serialize (nano::stream & stream_a) const
{
	if (!rsnano::rsn_message_confirm_req_serialize (handle, &stream_a))
	{
		throw std::runtime_error ("could not serialize confirm_req");
	}
}

bool nano::confirm_req::deserialize (nano::stream & stream_a, nano::block_uniquer * uniquer_a)
{
	rsnano::BlockUniquerHandle * uniquer_handle = nullptr;
	if (uniquer_a != nullptr)
	{
		uniquer_handle = uniquer_a->handle;
	}

	bool error = !rsnano::rsn_message_confirm_req_deserialize (handle, &stream_a, uniquer_handle);
	return error;
}

bool nano::confirm_req::operator== (nano::confirm_req const & other_a) const
{
	return rsnano::rsn_message_confirm_req_equals (handle, other_a.handle);
}

std::string nano::confirm_req::roots_string () const
{
	rsnano::StringDto dto;
	rsnano::rsn_message_confirm_req_roots_string (handle, &dto);
	return rsnano::convert_dto_to_string (dto);
}

std::size_t nano::confirm_req::size (nano::block_type type_a, std::size_t count)
{
	return rsnano::rsn_message_confirm_req_size (static_cast<uint8_t> (type_a), count);
}

rsnano::MessageHandle * create_confirm_ack_handle (nano::network_constants const & constants, nano::vote const & vote_a)
{
	auto constants_dto{ constants.to_dto () };
	return rsnano::rsn_message_confirm_ack_create (&constants_dto, vote_a.get_handle ());
}

rsnano::MessageHandle * create_confirm_ack_handle (bool & error_a, nano::stream & stream_a, nano::message_header const & header_a, nano::vote_uniquer * uniquer_a)
{
	rsnano::VoteUniquerHandle * uniquer_handle = nullptr;
	if (uniquer_a != nullptr)
	{
		uniquer_handle = uniquer_a->handle;
	}
	return rsnano::rsn_message_confirm_ack_create2 (header_a.handle, &stream_a, uniquer_handle, &error_a);
}

nano::confirm_ack::confirm_ack (bool & error_a, nano::stream & stream_a, nano::message_header const & header_a, nano::vote_uniquer * uniquer_a) :
	message (create_confirm_ack_handle (error_a, stream_a, header_a, uniquer_a))
{
}

nano::confirm_ack::confirm_ack (nano::network_constants const & constants, std::shared_ptr<nano::vote> const & vote_a) :
	message (create_confirm_ack_handle (constants, *vote_a))
{
}

nano::confirm_ack::confirm_ack (nano::confirm_ack const & other_a) :
	message (rsnano::rsn_message_confirm_ack_clone (other_a.handle))
{
}

void nano::confirm_ack::serialize (nano::stream & stream_a) const
{
	if (!rsnano::rsn_message_confirm_ack_serialize (handle, &stream_a))
	{
		throw std::runtime_error ("could not serialize confirm_ack");
	}
}

bool nano::confirm_ack::operator== (nano::confirm_ack const & other_a) const
{
	auto result (*get_vote () == *other_a.get_vote ());
	return result;
}

void nano::confirm_ack::visit (nano::message_visitor & visitor_a) const
{
	visitor_a.confirm_ack (*this);
}

std::size_t nano::confirm_ack::size (std::size_t count)
{
	return rsnano::rsn_message_confirm_ack_size (count);
}

std::shared_ptr<nano::vote> nano::confirm_ack::get_vote () const
{
	auto vote_handle{ rsnano::rsn_message_confirm_ack_vote (handle) };
	std::shared_ptr<nano::vote> result;
	if (vote_handle != nullptr)
	{
		result = std::make_shared<nano::vote> (vote_handle);
	}
	return result;
}

rsnano::MessageHandle * create_frontier_req_handle (nano::network_constants const & constants)
{
	auto constants_dto{ constants.to_dto () };
	return rsnano::rsn_message_frontier_req_create (&constants_dto);
}

nano::frontier_req::frontier_req (nano::network_constants const & constants) :
	message (create_frontier_req_handle (constants))
{
}

nano::frontier_req::frontier_req (bool & error_a, nano::stream & stream_a, nano::message_header const & header_a) :
	message (rsnano::rsn_message_frontier_req_create2 (header_a.handle))
{
	if (!error_a)
	{
		error_a = deserialize (stream_a);
	}
}

void nano::frontier_req::serialize (nano::stream & stream_a) const
{
	if (!rsnano::rsn_message_frontier_req_serialize (handle, &stream_a))
		throw std::runtime_error ("could not serialize frontier_req");
}

bool nano::frontier_req::deserialize (nano::stream & stream_a)
{
	bool error = !rsnano::rsn_message_frontier_req_deserialize (handle, &stream_a);
	return error;
}

void nano::frontier_req::visit (nano::message_visitor & visitor_a) const
{
	visitor_a.frontier_req (*this);
}

bool nano::frontier_req::operator== (nano::frontier_req const & other_a) const
{
	return get_start () == other_a.get_start () && get_age () == other_a.get_age () && get_count () == other_a.get_count ();
}

bool nano::frontier_req::is_only_confirmed_present () const
{
	return rsnano::rsn_message_frontier_req_is_confirmed_present (handle);
}

nano::account nano::frontier_req::get_start () const
{
	nano::account start;
	rsnano::rsn_message_frontier_req_start (handle, start.bytes.data ());
	return start;
}

uint32_t nano::frontier_req::get_age () const
{
	return rsnano::rsn_message_frontier_req_age (handle);
}

uint32_t nano::frontier_req::get_count () const
{
	return rsnano::rsn_message_frontier_req_count (handle);
}

void nano::frontier_req::set_start (nano::account const & account)
{
	rsnano::rsn_message_frontier_req_set_start (handle, account.bytes.data ());
}

void nano::frontier_req::set_age (uint32_t age_a)
{
	rsnano::rsn_message_frontier_req_set_age (handle, age_a);
}

void nano::frontier_req::set_count (uint32_t count_a)
{
	rsnano::rsn_message_frontier_req_set_count (handle, count_a);
}

std::size_t nano::frontier_req::size ()
{
	return rsnano::rsn_message_frontier_size ();
}

rsnano::MessageHandle * create_bulk_pull_handle (nano::network_constants const & constants)
{
	auto constants_dto{ constants.to_dto () };
	return rsnano::rsn_message_bulk_pull_create (&constants_dto);
}

nano::bulk_pull::bulk_pull (nano::network_constants const & constants) :
	message (create_bulk_pull_handle (constants))
{
}

nano::bulk_pull::bulk_pull (bool & error_a, nano::stream & stream_a, nano::message_header const & header_a) :
	message (rsnano::rsn_message_bulk_pull_create2 (header_a.handle))
{
	if (!error_a)
	{
		error_a = deserialize (stream_a);
	}
}

std::size_t nano::bulk_pull::size ()
{
	return rsnano::rsn_message_bulk_pull_size ();
}

nano::hash_or_account nano::bulk_pull::get_start () const
{
	nano::hash_or_account start;
	rsnano::rsn_message_bulk_pull_start (handle, start.bytes.data ());
	return start;
}

nano::block_hash nano::bulk_pull::get_end () const
{
	nano::block_hash end;
	rsnano::rsn_message_bulk_pull_end (handle, end.bytes.data ());
	return end;
}

uint32_t nano::bulk_pull::get_count () const
{
	return rsnano::rsn_message_bulk_pull_count (handle);
}

void nano::bulk_pull::set_start (nano::hash_or_account start_a)
{
	rsnano::rsn_message_bulk_pull_set_start (handle, start_a.bytes.data ());
}

void nano::bulk_pull::set_end (nano::block_hash end_a)
{
	rsnano::rsn_message_bulk_pull_set_end (handle, end_a.bytes.data ());
}

void nano::bulk_pull::set_count (uint32_t count_a)
{
	rsnano::rsn_message_bulk_pull_set_count (handle, count_a);
}

void nano::bulk_pull::visit (nano::message_visitor & visitor_a) const
{
	visitor_a.bulk_pull (*this);
}

void nano::bulk_pull::serialize (nano::stream & stream_a) const
{
	if (!rsnano::rsn_message_bulk_pull_serialize (handle, &stream_a))
		throw std::runtime_error ("could not serialize bulk_pull");
}

bool nano::bulk_pull::deserialize (nano::stream & stream_a)
{
	bool error = !rsnano::rsn_message_bulk_pull_deserialize (handle, &stream_a);
	return error;
}

bool nano::bulk_pull::is_count_present () const
{
	return rsnano::rsn_message_bulk_pull_is_count_present (handle);
}

void nano::bulk_pull::set_count_present (bool value_a)
{
	auto header{ get_header () };
	header.set_extension (count_present_flag, value_a);
	set_header (header);
}

rsnano::MessageHandle * create_bulk_pull_account_handle (nano::network_constants const & constants)
{
	auto constants_dto{ constants.to_dto () };
	return rsnano::rsn_message_bulk_pull_account_create (&constants_dto);
}

nano::bulk_pull_account::bulk_pull_account (nano::network_constants const & constants) :
	message (create_bulk_pull_account_handle (constants))
{
}

nano::bulk_pull_account::bulk_pull_account (bool & error_a, nano::stream & stream_a, nano::message_header const & header_a) :
	message (rsnano::rsn_message_bulk_pull_account_create2 (header_a.handle))
{
	if (!error_a)
	{
		error_a = deserialize (stream_a);
	}
}

void nano::bulk_pull_account::visit (nano::message_visitor & visitor_a) const
{
	visitor_a.bulk_pull_account (*this);
}

std::size_t nano::bulk_pull_account::size ()
{
	return rsnano::rsn_message_bulk_pull_account_size ();
}

nano::account nano::bulk_pull_account::get_account () const
{
	nano::account account;
	rsnano::rsn_message_bulk_pull_account_account (handle, account.bytes.data ());
	return account;
}

nano::amount nano::bulk_pull_account::get_minimum_amount () const
{
	nano::amount amount;
	rsnano::rsn_message_bulk_pull_account_minimum_amount (handle, amount.bytes.data ());
	return amount;
}

nano::bulk_pull_account_flags nano::bulk_pull_account::get_flags () const
{
	return static_cast<nano::bulk_pull_account_flags> (rsnano::rsn_message_bulk_pull_account_flags (handle));
}

void nano::bulk_pull_account::set_account (nano::account account_a)
{
	rsnano::rsn_message_bulk_pull_account_set_account (handle, account_a.bytes.data ());
}

void nano::bulk_pull_account::set_minimum_amount (nano::amount amount_a)
{
	rsnano::rsn_message_bulk_pull_account_set_minimum_amount (handle, amount_a.bytes.data ());
}

void nano::bulk_pull_account::set_flags (nano::bulk_pull_account_flags flags_a)
{
	rsnano::rsn_message_bulk_pull_account_set_flags (handle, static_cast<uint8_t> (flags_a));
}

void nano::bulk_pull_account::serialize (nano::stream & stream_a) const
{
	if (!rsnano::rsn_message_bulk_pull_account_serialize (handle, &stream_a))
		throw std::runtime_error ("bulk_pull_account could not be serialized");
}

bool nano::bulk_pull_account::deserialize (nano::stream & stream_a)
{
	bool error = !rsnano::rsn_message_bulk_pull_account_deserialize (handle, &stream_a);
	return error;
}

rsnano::MessageHandle * create_bulk_push_handle (nano::network_constants const & constants)
{
	auto constants_dto{ constants.to_dto () };
	return rsnano::rsn_message_bulk_push_create (&constants_dto);
}

nano::bulk_push::bulk_push (nano::network_constants const & constants) :
	message (create_bulk_push_handle (constants))
{
}

nano::bulk_push::bulk_push (nano::message_header const & header_a) :
	message (rsnano::rsn_message_bulk_push_create2 (header_a.handle))
{
}

bool nano::bulk_push::deserialize (nano::stream & stream_a)
{
	bool error = !rsnano::rsn_message_bulk_push_deserialize (handle, &stream_a);
	return error;
}

void nano::bulk_push::serialize (nano::stream & stream_a) const
{
	if (!rsnano::rsn_message_bulk_push_serialize (handle, &stream_a))
		throw std::runtime_error ("could not serialize bulk_push");
}

void nano::bulk_push::visit (nano::message_visitor & visitor_a) const
{
	visitor_a.bulk_push (*this);
}

rsnano::MessageHandle * create_telemetry_req_handle (nano::network_constants const & constants)
{
	auto constants_dto{ constants.to_dto () };
	return rsnano::rsn_message_telemetry_req_create (&constants_dto);
}

nano::telemetry_req::telemetry_req (nano::network_constants const & constants) :
	message (create_telemetry_req_handle (constants))
{
}

nano::telemetry_req::telemetry_req (nano::message_header const & header_a) :
	message (rsnano::rsn_message_telemetry_req_create2 (header_a.handle))
{
}

nano::telemetry_req::telemetry_req (nano::telemetry_req const & other_a) :
	message (rsnano::rsn_message_telemetry_req_clone (other_a.handle))
{
}

bool nano::telemetry_req::deserialize (nano::stream & stream_a)
{
	bool error = !rsnano::rsn_message_telemetry_req_deserialize (handle, &stream_a);
	return error;
}

void nano::telemetry_req::serialize (nano::stream & stream_a) const
{
	if (!rsnano::rsn_message_telemetry_req_serialize (handle, &stream_a))
		throw std::runtime_error ("could not serialize telemetry_req");
}

void nano::telemetry_req::visit (nano::message_visitor & visitor_a) const
{
	visitor_a.telemetry_req (*this);
}

rsnano::MessageHandle * create_telemetry_ack_handle (nano::network_constants const & constants)
{
	auto constants_dto{ constants.to_dto () };
	return rsnano::rsn_message_telemetry_ack_create (&constants_dto);
}

nano::telemetry_ack::telemetry_ack (nano::network_constants const & constants) :
	message (create_telemetry_ack_handle (constants))
{
}

nano::telemetry_ack::telemetry_ack (bool & error_a, nano::stream & stream_a, nano::message_header const & message_header) :
	message (rsnano::rsn_message_telemetry_ack_create2 (message_header.handle))
{
	if (!error_a)
	{
		error_a = deserialize (stream_a);
	}
}

nano::telemetry_ack::telemetry_ack (nano::telemetry_ack const & other_a) :
	message (rsnano::rsn_message_telemetry_ack_clone (other_a.handle)),
	data{ other_a.data }
{
}

nano::telemetry_ack::telemetry_ack (nano::network_constants const & constants, nano::telemetry_data const & telemetry_data_a) :
	message (create_telemetry_ack_handle (constants)),
	data (telemetry_data_a)
{
	debug_assert (telemetry_data::size + telemetry_data_a.get_unknown_data ().size () <= message_header::telemetry_size_mask.to_ulong ()); // Maximum size the mask allows
	auto header{ get_header () };
	auto extensions{ header.get_extensions () };
	extensions &= ~message_header::telemetry_size_mask;
	extensions |= std::bitset<16> (static_cast<unsigned long long> (telemetry_data::size) + telemetry_data_a.get_unknown_data ().size ());
	header.set_extensions (extensions);
	set_header (header);
}

nano::telemetry_ack & nano::telemetry_ack::operator= (telemetry_ack const & other_a)
{
	handle = rsnano::rsn_message_telemetry_ack_clone (other_a.handle);
	data = other_a.data;
	return *this;
}

void nano::telemetry_ack::serialize (nano::stream & stream_a) const
{
	get_header ().serialize (stream_a);
	if (!is_empty_payload ())
	{
		data.serialize (stream_a);
	}
}

bool nano::telemetry_ack::deserialize (nano::stream & stream_a)
{
	auto error (false);
	debug_assert (get_header ().get_type () == nano::message_type::telemetry_ack);
	try
	{
		if (!is_empty_payload ())
		{
			data.deserialize (stream_a, nano::narrow_cast<uint16_t> (get_header ().get_extensions ().to_ulong ()));
		}
	}
	catch (std::runtime_error const &)
	{
		error = true;
	}

	return error;
}

void nano::telemetry_ack::visit (nano::message_visitor & visitor_a) const
{
	visitor_a.telemetry_ack (*this);
}

uint16_t nano::telemetry_ack::size () const
{
	return size (get_header ());
}

uint16_t nano::telemetry_ack::size (nano::message_header const & message_header_a)
{
	return static_cast<uint16_t> ((message_header_a.get_extensions () & message_header::telemetry_size_mask).to_ullong ());
}

bool nano::telemetry_ack::is_empty_payload () const
{
	return size () == 0;
}

nano::telemetry_data::telemetry_data () :
	handle{ rsnano::rsn_telemetry_data_create () }
{
}

nano::telemetry_data::telemetry_data (nano::telemetry_data const & other_a) :
	handle{ rsnano::rsn_telemetry_data_clone (other_a.handle) }
{
	signature = other_a.signature;
	node_id = other_a.node_id;
	block_count = other_a.block_count;
	cemented_count = other_a.cemented_count;
	unchecked_count = other_a.unchecked_count;
	account_count = other_a.account_count;
	bandwidth_cap = other_a.bandwidth_cap;
	uptime = other_a.uptime;
	peer_count = other_a.peer_count;
	protocol_version = other_a.protocol_version;
	genesis_block = other_a.genesis_block;
	major_version = other_a.major_version;
	minor_version = other_a.minor_version;
	patch_version = other_a.patch_version;
	pre_release_version = other_a.pre_release_version;
	maker = other_a.maker;
	timestamp = other_a.timestamp;
	active_difficulty = other_a.active_difficulty;
	unknown_data = other_a.unknown_data;
}

nano::telemetry_data::telemetry_data (nano::telemetry_data && other_a) :
	handle{ other_a.handle }
{
	other_a.handle = nullptr;
	signature = other_a.signature;
	node_id = other_a.node_id;
	block_count = other_a.block_count;
	cemented_count = other_a.cemented_count;
	unchecked_count = other_a.unchecked_count;
	account_count = other_a.account_count;
	bandwidth_cap = other_a.bandwidth_cap;
	uptime = other_a.uptime;
	peer_count = other_a.peer_count;
	protocol_version = other_a.protocol_version;
	genesis_block = other_a.genesis_block;
	major_version = other_a.major_version;
	minor_version = other_a.minor_version;
	patch_version = other_a.patch_version;
	pre_release_version = other_a.pre_release_version;
	maker = other_a.maker;
	timestamp = other_a.timestamp;
	active_difficulty = other_a.active_difficulty;
	unknown_data = other_a.unknown_data;
}

nano::telemetry_data::~telemetry_data ()
{
	if (handle != nullptr)
		rsnano::rsn_telemetry_data_destroy (handle);
}

nano::telemetry_data & nano::telemetry_data::operator= (nano::telemetry_data const & other_a)
{
	signature = other_a.signature;
	node_id = other_a.node_id;
	block_count = other_a.block_count;
	cemented_count = other_a.cemented_count;
	unchecked_count = other_a.unchecked_count;
	account_count = other_a.account_count;
	bandwidth_cap = other_a.bandwidth_cap;
	uptime = other_a.uptime;
	peer_count = other_a.peer_count;
	protocol_version = other_a.protocol_version;
	genesis_block = other_a.genesis_block;
	major_version = other_a.major_version;
	minor_version = other_a.minor_version;
	patch_version = other_a.patch_version;
	pre_release_version = other_a.pre_release_version;
	maker = other_a.maker;
	timestamp = other_a.timestamp;
	active_difficulty = other_a.active_difficulty;
	unknown_data = other_a.unknown_data;
	return *this;
}

nano::signature nano::telemetry_data::get_signature () const
{
	return signature;
}

void nano::telemetry_data::set_signature (nano::signature const & signature_a)
{
	signature = signature_a;
}

nano::account nano::telemetry_data::get_node_id () const
{
	return node_id;
}

void nano::telemetry_data::set_node_id (nano::account const & node_id_a)
{
	node_id = node_id_a;
}

uint64_t nano::telemetry_data::get_block_count () const
{
	return block_count;
}

void nano::telemetry_data::set_block_count (uint64_t count_a)
{
	block_count = count_a;
}

uint64_t nano::telemetry_data::get_cemented_count () const
{
	return cemented_count;
}
void nano::telemetry_data::set_cemented_count (uint64_t count_a)
{
	cemented_count = count_a;
}
uint64_t nano::telemetry_data::get_unchecked_count () const
{
	return unchecked_count;
}
void nano::telemetry_data::set_unchecked_count (uint64_t count_a)
{
	unchecked_count = count_a;
}
uint64_t nano::telemetry_data::get_account_count () const
{
	return account_count;
}
void nano::telemetry_data::set_account_count (uint64_t count_a)
{
	account_count = count_a;
}
uint64_t nano::telemetry_data::get_bandwidth_cap () const
{
	return bandwidth_cap;
}
void nano::telemetry_data::set_bandwidth_cap (uint64_t cap_a)
{
	bandwidth_cap = cap_a;
}
uint64_t nano::telemetry_data::get_uptime () const
{
	return uptime;
}
void nano::telemetry_data::set_uptime (uint64_t uptime_a)
{
	uptime = uptime_a;
}
uint32_t nano::telemetry_data::get_peer_count () const
{
	return peer_count;
}
void nano::telemetry_data::set_peer_count (uint32_t count_a)
{
	peer_count = count_a;
}
uint8_t nano::telemetry_data::get_protocol_version () const
{
	return protocol_version;
}
void nano::telemetry_data::set_protocol_version (uint8_t version_a)
{
	protocol_version = version_a;
}
nano::block_hash nano::telemetry_data::get_genesis_block () const
{
	return genesis_block;
}
void nano::telemetry_data::set_genesis_block (nano::block_hash const & block_a)
{
	genesis_block = block_a;
}
uint8_t nano::telemetry_data::get_major_version () const
{
	return major_version;
}
void nano::telemetry_data::set_major_version (uint8_t version_a)
{
	major_version = version_a;
}
uint8_t nano::telemetry_data::get_minor_version () const
{
	return minor_version;
}
void nano::telemetry_data::set_minor_version (uint8_t version_a)
{
	minor_version = version_a;
}
uint8_t nano::telemetry_data::get_patch_version () const
{
	return patch_version;
}
void nano::telemetry_data::set_patch_version (uint8_t version_a)
{
	patch_version = version_a;
}
uint8_t nano::telemetry_data::get_pre_release_version () const
{
	return pre_release_version;
}
void nano::telemetry_data::set_pre_release_version (uint8_t version_a)
{
	pre_release_version = version_a;
}
uint8_t nano::telemetry_data::get_maker () const
{
	return maker;
}
void nano::telemetry_data::set_maker (uint8_t maker_a)
{
	maker = maker_a;
}
std::chrono::system_clock::time_point nano::telemetry_data::get_timestamp () const
{
	return timestamp;
}
void nano::telemetry_data::set_timestamp (std::chrono::system_clock::time_point timestamp_a)
{
	timestamp = timestamp_a;
}
uint64_t nano::telemetry_data::get_active_difficulty () const
{
	return active_difficulty;
}
void nano::telemetry_data::set_active_difficulty (uint64_t difficulty_a)
{
	active_difficulty = difficulty_a;
}
std::vector<uint8_t> nano::telemetry_data::get_unknown_data () const
{
	return unknown_data;
}
void nano::telemetry_data::set_unknown_data (std::vector<uint8_t> data_a)
{
	unknown_data = data_a;
}

void nano::telemetry_data::deserialize (nano::stream & stream_a, uint16_t payload_length_a)
{
	read (stream_a, signature);
	read (stream_a, node_id);
	read (stream_a, block_count);
	boost::endian::big_to_native_inplace (block_count);
	read (stream_a, cemented_count);
	boost::endian::big_to_native_inplace (cemented_count);
	read (stream_a, unchecked_count);
	boost::endian::big_to_native_inplace (unchecked_count);
	read (stream_a, account_count);
	boost::endian::big_to_native_inplace (account_count);
	read (stream_a, bandwidth_cap);
	boost::endian::big_to_native_inplace (bandwidth_cap);
	read (stream_a, peer_count);
	boost::endian::big_to_native_inplace (peer_count);
	read (stream_a, protocol_version);
	read (stream_a, uptime);
	boost::endian::big_to_native_inplace (uptime);
	read (stream_a, genesis_block.bytes);
	read (stream_a, major_version);
	read (stream_a, minor_version);
	read (stream_a, patch_version);
	read (stream_a, pre_release_version);
	read (stream_a, maker);

	uint64_t timestamp_l;
	read (stream_a, timestamp_l);
	boost::endian::big_to_native_inplace (timestamp_l);
	timestamp = std::chrono::system_clock::time_point (std::chrono::milliseconds (timestamp_l));
	read (stream_a, active_difficulty);
	boost::endian::big_to_native_inplace (active_difficulty);
	if (payload_length_a > latest_size)
	{
		read (stream_a, unknown_data, payload_length_a - latest_size);
	}
}

void nano::telemetry_data::serialize_without_signature (nano::stream & stream_a) const
{
	// All values should be serialized in big endian
	write (stream_a, node_id);
	write (stream_a, boost::endian::native_to_big (block_count));
	write (stream_a, boost::endian::native_to_big (cemented_count));
	write (stream_a, boost::endian::native_to_big (unchecked_count));
	write (stream_a, boost::endian::native_to_big (account_count));
	write (stream_a, boost::endian::native_to_big (bandwidth_cap));
	write (stream_a, boost::endian::native_to_big (peer_count));
	write (stream_a, protocol_version);
	write (stream_a, boost::endian::native_to_big (uptime));
	write (stream_a, genesis_block.bytes);
	write (stream_a, major_version);
	write (stream_a, minor_version);
	write (stream_a, patch_version);
	write (stream_a, pre_release_version);
	write (stream_a, maker);
	write (stream_a, boost::endian::native_to_big (std::chrono::duration_cast<std::chrono::milliseconds> (timestamp.time_since_epoch ()).count ()));
	write (stream_a, boost::endian::native_to_big (active_difficulty));
	write (stream_a, unknown_data);
}

void nano::telemetry_data::serialize (nano::stream & stream_a) const
{
	write (stream_a, signature);
	serialize_without_signature (stream_a);
}

nano::error nano::telemetry_data::serialize_json (nano::jsonconfig & json, bool ignore_identification_metrics_a) const
{
	json.put ("block_count", block_count);
	json.put ("cemented_count", cemented_count);
	json.put ("unchecked_count", unchecked_count);
	json.put ("account_count", account_count);
	json.put ("bandwidth_cap", bandwidth_cap);
	json.put ("peer_count", peer_count);
	json.put ("protocol_version", protocol_version);
	json.put ("uptime", uptime);
	json.put ("genesis_block", genesis_block.to_string ());
	json.put ("major_version", major_version);
	json.put ("minor_version", minor_version);
	json.put ("patch_version", patch_version);
	json.put ("pre_release_version", pre_release_version);
	json.put ("maker", maker);
	json.put ("timestamp", std::chrono::duration_cast<std::chrono::milliseconds> (timestamp.time_since_epoch ()).count ());
	json.put ("active_difficulty", nano::to_string_hex (active_difficulty));
	// Keep these last for UI purposes
	if (!ignore_identification_metrics_a)
	{
		json.put ("node_id", node_id.to_node_id ());
		json.put ("signature", signature.to_string ());
	}
	return json.get_error ();
}

nano::error nano::telemetry_data::deserialize_json (nano::jsonconfig & json, bool ignore_identification_metrics_a)
{
	if (!ignore_identification_metrics_a)
	{
		std::string signature_l;
		json.get ("signature", signature_l);
		if (!json.get_error ())
		{
			if (signature.decode_hex (signature_l))
			{
				json.get_error ().set ("Could not deserialize signature");
			}
		}

		std::string node_id_l;
		json.get ("node_id", node_id_l);
		if (!json.get_error ())
		{
			if (node_id.decode_node_id (node_id_l))
			{
				json.get_error ().set ("Could not deserialize node id");
			}
		}
	}

	json.get ("block_count", block_count);
	json.get ("cemented_count", cemented_count);
	json.get ("unchecked_count", unchecked_count);
	json.get ("account_count", account_count);
	json.get ("bandwidth_cap", bandwidth_cap);
	json.get ("peer_count", peer_count);
	json.get ("protocol_version", protocol_version);
	json.get ("uptime", uptime);
	std::string genesis_block_l;
	json.get ("genesis_block", genesis_block_l);
	if (!json.get_error ())
	{
		if (genesis_block.decode_hex (genesis_block_l))
		{
			json.get_error ().set ("Could not deserialize genesis block");
		}
	}
	json.get ("major_version", major_version);
	json.get ("minor_version", minor_version);
	json.get ("patch_version", patch_version);
	json.get ("pre_release_version", pre_release_version);
	json.get ("maker", maker);
	auto timestamp_l = json.get<uint64_t> ("timestamp");
	timestamp = std::chrono::system_clock::time_point (std::chrono::milliseconds (timestamp_l));
	auto current_active_difficulty_text = json.get<std::string> ("active_difficulty");
	auto ec = nano::from_string_hex (current_active_difficulty_text, active_difficulty);
	debug_assert (!ec);
	return json.get_error ();
}

std::string nano::telemetry_data::to_string () const
{
	nano::jsonconfig jc;
	serialize_json (jc, true);
	std::stringstream ss;
	jc.write (ss);
	return ss.str ();
}

bool nano::telemetry_data::operator== (nano::telemetry_data const & data_a) const
{
	return (signature == data_a.signature && node_id == data_a.node_id && block_count == data_a.block_count && cemented_count == data_a.cemented_count && unchecked_count == data_a.unchecked_count && account_count == data_a.account_count && bandwidth_cap == data_a.bandwidth_cap && uptime == data_a.uptime && peer_count == data_a.peer_count && protocol_version == data_a.protocol_version && genesis_block == data_a.genesis_block && major_version == data_a.major_version && minor_version == data_a.minor_version && patch_version == data_a.patch_version && pre_release_version == data_a.pre_release_version && maker == data_a.maker && timestamp == data_a.timestamp && active_difficulty == data_a.active_difficulty && unknown_data == data_a.unknown_data);
}

bool nano::telemetry_data::operator!= (nano::telemetry_data const & data_a) const
{
	return !(*this == data_a);
}

void nano::telemetry_data::sign (nano::keypair const & node_id_a)
{
	debug_assert (node_id == node_id_a.pub);
	std::vector<uint8_t> bytes;
	{
		nano::vectorstream stream (bytes);
		serialize_without_signature (stream);
	}

	signature = nano::sign_message (node_id_a.prv, node_id_a.pub, bytes.data (), bytes.size ());
}

bool nano::telemetry_data::validate_signature () const
{
	std::vector<uint8_t> bytes;
	{
		nano::vectorstream stream (bytes);
		serialize_without_signature (stream);
	}

	return nano::validate_message (node_id, bytes.data (), bytes.size (), signature);
}

rsnano::MessageHandle * create_node_id_handshake_handle (nano::network_constants const & constants)
{
	auto constants_dto{ constants.to_dto () };
	return rsnano::rsn_message_node_id_handshake_create (&constants_dto);
}

nano::node_id_handshake::node_id_handshake (bool & error_a, nano::stream & stream_a, nano::message_header const & header_a) :
	message (rsnano::rsn_message_node_id_handshake_create2 (header_a.handle)),
	query (boost::none),
	response (boost::none)
{
	error_a = deserialize (stream_a);
}

nano::node_id_handshake::node_id_handshake (node_id_handshake const & other_a) :
	message{ rsnano::rsn_message_node_id_handshake_clone (other_a.handle) },
	query (other_a.query),
	response (other_a.response)
{
}

nano::node_id_handshake::node_id_handshake (nano::network_constants const & constants, boost::optional<nano::uint256_union> query, boost::optional<std::pair<nano::account, nano::signature>> response) :
	message (create_node_id_handshake_handle (constants)),
	query (query),
	response (response)
{
	auto header{ get_header () };
	if (query)
	{
		header.flag_set (nano::message_header::node_id_handshake_query_flag);
	}
	if (response)
	{
		header.flag_set (nano::message_header::node_id_handshake_response_flag);
	}
	set_header (header);
}

void nano::node_id_handshake::serialize (nano::stream & stream_a) const
{
	get_header ().serialize (stream_a);
	if (query)
	{
		write (stream_a, *query);
	}
	if (response)
	{
		write (stream_a, response->first);
		write (stream_a, response->second);
	}
}

bool nano::node_id_handshake::deserialize (nano::stream & stream_a)
{
	auto header{ get_header () };
	debug_assert (header.get_type () == nano::message_type::node_id_handshake);
	auto error (false);
	try
	{
		if (header.node_id_handshake_is_query ())
		{
			nano::uint256_union query_hash;
			read (stream_a, query_hash);
			query = query_hash;
		}

		if (header.node_id_handshake_is_response ())
		{
			nano::account response_account;
			read (stream_a, response_account);
			nano::signature response_signature;
			read (stream_a, response_signature);
			response = std::make_pair (response_account, response_signature);
		}
	}
	catch (std::runtime_error const &)
	{
		error = true;
	}

	return error;
}

bool nano::node_id_handshake::operator== (nano::node_id_handshake const & other_a) const
{
	auto result (*query == *other_a.query && *response == *other_a.response);
	return result;
}

void nano::node_id_handshake::visit (nano::message_visitor & visitor_a) const
{
	visitor_a.node_id_handshake (*this);
}

std::size_t nano::node_id_handshake::size () const
{
	return size (get_header ());
}

std::size_t nano::node_id_handshake::size (nano::message_header const & header_a)
{
	std::size_t result (0);
	if (header_a.node_id_handshake_is_query ())
	{
		result = sizeof (nano::uint256_union);
	}
	if (header_a.node_id_handshake_is_response ())
	{
		result += sizeof (nano::account) + sizeof (nano::signature);
	}
	return result;
}

nano::message_visitor::~message_visitor ()
{
}

bool nano::parse_port (std::string const & string_a, uint16_t & port_a)
{
	bool result = false;
	try
	{
		port_a = boost::lexical_cast<uint16_t> (string_a);
	}
	catch (...)
	{
		result = true;
	}
	return result;
}

// Can handle both ipv4 & ipv6 addresses (with and without square brackets)
bool nano::parse_address (std::string const & address_text_a, boost::asio::ip::address & address_a)
{
	auto address_text = address_text_a;
	if (!address_text.empty () && address_text.front () == '[' && address_text.back () == ']')
	{
		// Chop the square brackets off as make_address doesn't always like them
		address_text = address_text.substr (1, address_text.size () - 2);
	}

	boost::system::error_code address_ec;
	address_a = boost::asio::ip::make_address (address_text, address_ec);
	return !!address_ec;
}

bool nano::parse_address_port (std::string const & string, boost::asio::ip::address & address_a, uint16_t & port_a)
{
	auto result (false);
	auto port_position (string.rfind (':'));
	if (port_position != std::string::npos && port_position > 0)
	{
		std::string port_string (string.substr (port_position + 1));
		try
		{
			uint16_t port;
			result = parse_port (port_string, port);
			if (!result)
			{
				boost::system::error_code ec;
				auto address (boost::asio::ip::make_address_v6 (string.substr (0, port_position), ec));
				if (!ec)
				{
					address_a = address;
					port_a = port;
				}
				else
				{
					result = true;
				}
			}
			else
			{
				result = true;
			}
		}
		catch (...)
		{
			result = true;
		}
	}
	else
	{
		result = true;
	}
	return result;
}

bool nano::parse_endpoint (std::string const & string, nano::endpoint & endpoint_a)
{
	boost::asio::ip::address address;
	uint16_t port;
	auto result (parse_address_port (string, address, port));
	if (!result)
	{
		endpoint_a = nano::endpoint (address, port);
	}
	return result;
}

bool nano::parse_tcp_endpoint (std::string const & string, nano::tcp_endpoint & endpoint_a)
{
	boost::asio::ip::address address;
	uint16_t port;
	auto result (parse_address_port (string, address, port));
	if (!result)
	{
		endpoint_a = nano::tcp_endpoint (address, port);
	}
	return result;
}

std::chrono::seconds nano::telemetry_cache_cutoffs::network_to_time (network_constants const & network_constants)
{
	return std::chrono::seconds{ (network_constants.is_live_network () || network_constants.is_test_network ()) ? live : network_constants.is_beta_network () ? beta
																																							  : dev };
}

nano::node_singleton_memory_pool_purge_guard::node_singleton_memory_pool_purge_guard () :
	cleanup_guard ({ nano::block_memory_pool_purge, nano::purge_shared_ptr_singleton_pool_memory<nano::vote>, nano::purge_shared_ptr_singleton_pool_memory<nano::election>, nano::purge_singleton_inactive_votes_cache_pool_memory })
{
}
