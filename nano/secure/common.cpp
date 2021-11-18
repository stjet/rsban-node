#include <nano/crypto_lib/random_pool.hpp>
#include <nano/lib/config.hpp>
#include <nano/lib/numbers.hpp>
#include <nano/lib/rsnano_callbacks.hpp>
#include <nano/lib/timer.hpp>
#include <nano/secure/common.hpp>
#include <nano/secure/store.hpp>

#include <crypto/cryptopp/words.h>

#include <boost/endian/conversion.hpp>
#include <boost/property_tree/json_parser.hpp>
#include <boost/variant/get.hpp>

#include <limits>
#include <queue>

#include <crypto/ed25519-donna/ed25519.h>

namespace
{
char const * dev_private_key_data = "34F0A37AAD20F4A260F0A5B3CB3D7FB50673212263E58A380BC10474BB039CE4";
}

nano::keypair nano::dev::genesis_key{ dev_private_key_data };
nano::network_params nano::dev::network_params{ nano::networks::nano_dev_network };
nano::ledger_constants & nano::dev::constants{ nano::dev::network_params.ledger };
std::shared_ptr<nano::block> & nano::dev::genesis = nano::dev::constants.genesis;

nano::network_params::network_params (nano::networks network_a)
{
	rsnano::NetworkParamsDto dto;
	if (rsnano::rsn_network_params_create (&dto, static_cast<uint16_t> (network_a)) < 0)
		throw std::runtime_error ("could not create network params");

	work = nano::work_thresholds (dto.work);
	network = nano::network_constants (dto.network);
	ledger = nano::ledger_constants (dto.ledger);
	voting = nano::voting_constants (dto.voting);
	node = nano::node_constants (dto.node);
	portmapping = nano::portmapping_constants (dto.portmapping);
	bootstrap = nano::bootstrap_constants (dto.bootstrap);
	kdf_work = dto.kdf_work;
}

nano::ledger_constants::ledger_constants (nano::work_thresholds & work_a, nano::networks network_a)
{
	rsnano::LedgerConstantsDto dto;
	if (rsnano::rsn_ledger_constants_create (&dto, &work_a.dto, static_cast<uint16_t> (network_a)) < 0)
		throw std::runtime_error ("could not create ledger_constants");
	read_dto (dto);
}

nano::ledger_constants::ledger_constants (rsnano::LedgerConstantsDto const & dto)
{
	read_dto (dto);
}

void nano::ledger_constants::read_dto (rsnano::LedgerConstantsDto const & dto)
{
	work = nano::work_thresholds (dto.work);
	nano::public_key pub_key;
	nano::raw_key priv_key;
	std::copy (std::begin (dto.pub_key), std::end (dto.pub_key), std::begin (pub_key.bytes));
	std::copy (std::begin (dto.priv_key), std::end (dto.priv_key), std::begin (priv_key.bytes));
	zero_key = nano::keypair (priv_key, pub_key);
	std::copy (std::begin (dto.nano_beta_account), std::end (dto.nano_beta_account), std::begin (nano_beta_account.bytes));
	std::copy (std::begin (dto.nano_live_account), std::end (dto.nano_live_account), std::begin (nano_live_account.bytes));
	std::copy (std::begin (dto.nano_test_account), std::end (dto.nano_test_account), std::begin (nano_test_account.bytes));
	nano_dev_genesis = nano::block_dto_to_block (dto.nano_dev_genesis);
	nano_beta_genesis = nano::block_dto_to_block (dto.nano_beta_genesis);
	nano_live_genesis = nano::block_dto_to_block (dto.nano_live_genesis);
	nano_test_genesis = nano::block_dto_to_block (dto.nano_test_genesis);
	genesis = nano::block_dto_to_block (dto.genesis);
	boost::multiprecision::import_bits (genesis_amount, std::begin (dto.genesis_amount), std::end (dto.genesis_amount));
	std::copy (std::begin (dto.burn_account), std::end (dto.burn_account), std::begin (burn_account.bytes));
	std::copy (std::begin (dto.nano_dev_final_votes_canary_account), std::end (dto.nano_dev_final_votes_canary_account), std::begin (nano_dev_final_votes_canary_account.bytes));
	std::copy (std::begin (dto.nano_beta_final_votes_canary_account), std::end (dto.nano_beta_final_votes_canary_account), std::begin (nano_beta_final_votes_canary_account.bytes));
	std::copy (std::begin (dto.nano_live_final_votes_canary_account), std::end (dto.nano_live_final_votes_canary_account), std::begin (nano_live_final_votes_canary_account.bytes));
	std::copy (std::begin (dto.nano_test_final_votes_canary_account), std::end (dto.nano_test_final_votes_canary_account), std::begin (nano_test_final_votes_canary_account.bytes));
	std::copy (std::begin (dto.final_votes_canary_account), std::end (dto.final_votes_canary_account), std::begin (final_votes_canary_account.bytes));
	nano_dev_final_votes_canary_height = dto.nano_dev_final_votes_canary_height;
	nano_beta_final_votes_canary_height = dto.nano_beta_final_votes_canary_height;
	nano_live_final_votes_canary_height = dto.nano_live_final_votes_canary_height;
	nano_test_final_votes_canary_height = dto.nano_test_final_votes_canary_height;
	final_votes_canary_height = dto.final_votes_canary_height;

	nano::account epoch_v1_signer;
	std::copy (std::begin (dto.epoch_1_signer), std::end (dto.epoch_1_signer), std::begin (epoch_v1_signer.bytes));
	nano::link epoch_v1_link;
	std::copy (std::begin (dto.epoch_1_link), std::end (dto.epoch_1_link), std::begin (epoch_v1_link.bytes));
	nano::account epoch_v2_signer;
	std::copy (std::begin (dto.epoch_2_signer), std::end (dto.epoch_2_signer), std::begin (epoch_v2_signer.bytes));
	nano::link epoch_v2_link;
	std::copy (std::begin (dto.epoch_2_link), std::end (dto.epoch_2_link), std::begin (epoch_v2_link.bytes));

	epochs.add (nano::epoch::epoch_1, epoch_v1_signer, epoch_v1_link);
	epochs.add (nano::epoch::epoch_2, epoch_v2_signer, epoch_v2_link);
}

nano::hardened_constants & nano::hardened_constants::get ()
{
	static hardened_constants instance{};
	return instance;
}

nano::hardened_constants::hardened_constants () :
	not_an_account{},
	random_128{}
{
	nano::random_pool::generate_block (not_an_account.bytes.data (), not_an_account.bytes.size ());
	nano::random_pool::generate_block (random_128.bytes.data (), random_128.bytes.size ());
}

nano::node_constants::node_constants (nano::network_constants & network_constants)
{
	rsnano::NodeConstantsDto dto;
	auto network_dto{ network_constants.to_dto () };
	if (rsnano::rsn_node_constants_create (&network_dto, &dto) < 0)
		throw std::runtime_error ("could not create node constants");
	read_dto (dto);
}

nano::node_constants::node_constants (rsnano::NodeConstantsDto const & dto)
{
	read_dto (dto);
}

void nano::node_constants::read_dto (rsnano::NodeConstantsDto const & dto)
{
	backup_interval = std::chrono::minutes (dto.backup_interval_m);
	search_pending_interval = std::chrono::seconds (dto.search_pending_interval_s);
	unchecked_cleaning_interval = std::chrono::minutes (dto.unchecked_cleaning_interval_m);
	process_confirmed_interval = std::chrono::milliseconds (dto.process_confirmed_interval_ms);
	max_weight_samples = dto.max_weight_samples;
	weight_period = dto.weight_period;
}

nano::voting_constants::voting_constants (nano::network_constants & network_constants)
{
	auto network_dto{ network_constants.to_dto () };
	rsnano::VotingConstantsDto dto;
	if (rsnano::rsn_voting_constants_create (&network_dto, &dto) < 0)
		throw std::runtime_error ("could not create voting constants");
	max_cache = dto.max_cache;
	delay = std::chrono::seconds (dto.delay_s);
}

nano::voting_constants::voting_constants (rsnano::VotingConstantsDto const & dto)
{
	max_cache = dto.max_cache;
	delay = std::chrono::seconds (dto.delay_s);
}

nano::portmapping_constants::portmapping_constants (nano::network_constants & network_constants)
{
	rsnano::PortmappingConstantsDto dto;
	auto network_dto{ network_constants.to_dto () };
	if (rsnano::rsn_portmapping_constants_create (&network_dto, &dto) < 0)
		throw std::runtime_error ("could not create portmapping constants");
	lease_duration = std::chrono::seconds (dto.lease_duration_s);
	health_check_period = std::chrono::seconds (dto.health_check_period_s);
}

nano::portmapping_constants::portmapping_constants (rsnano::PortmappingConstantsDto const & dto)
{
	lease_duration = std::chrono::seconds (dto.lease_duration_s);
	health_check_period = std::chrono::seconds (dto.health_check_period_s);
}

nano::bootstrap_constants::bootstrap_constants (nano::network_constants & network_constants)
{
	auto network_dto{ network_constants.to_dto () };
	rsnano::BootstrapConstantsDto dto;
	if (rsnano::rsn_bootstrap_constants_create (&network_dto, &dto) < 0)
		throw std::runtime_error ("could not create bootstrap constants");
	read_dto (dto);
}

nano::bootstrap_constants::bootstrap_constants (rsnano::BootstrapConstantsDto const & dto)
{
	read_dto (dto);
}

void nano::bootstrap_constants::read_dto (rsnano::BootstrapConstantsDto const & dto)
{
	lazy_max_pull_blocks = dto.lazy_max_pull_blocks;
	lazy_min_pull_blocks = dto.lazy_min_pull_blocks;
	frontier_retry_limit = dto.frontier_retry_limit;
	lazy_retry_limit = dto.lazy_retry_limit;
	lazy_destinations_retry_limit = dto.lazy_destinations_retry_limit;
	gap_cache_bootstrap_start_interval = std::chrono::milliseconds (dto.gap_cache_bootstrap_start_interval_ms);
	default_frontiers_age_seconds = dto.default_frontiers_age_seconds;
}

// Create a new random keypair
nano::keypair::keypair ()
{
	random_pool::generate_block (prv.bytes.data (), prv.bytes.size ());
	ed25519_publickey (prv.bytes.data (), pub.bytes.data ());
}

// Create a keypair given a private key
nano::keypair::keypair (nano::raw_key && prv_a) :
	prv (std::move (prv_a))
{
	ed25519_publickey (prv.bytes.data (), pub.bytes.data ());
}

// Create a keypair given a hex string of the private key
nano::keypair::keypair (std::string const & prv_a)
{
	[[maybe_unused]] auto error (prv.decode_hex (prv_a));
	debug_assert (!error);
	ed25519_publickey (prv.bytes.data (), pub.bytes.data ());
}

nano::keypair::keypair (nano::raw_key const & priv_key_a, nano::public_key const & pub_key_a) :
	prv (priv_key_a),
	pub (pub_key_a)
{
}

// Serialize a block prefixed with an 8-bit typecode
void nano::serialize_block (nano::stream & stream_a, nano::block const & block_a)
{
	write (stream_a, block_a.type ());
	block_a.serialize (stream_a);
}

nano::account_info::account_info (nano::block_hash const & head_a, nano::account const & representative_a, nano::block_hash const & open_block_a, nano::amount const & balance_a, uint64_t modified_a, uint64_t block_count_a, nano::epoch epoch_a) :
	head (head_a),
	representative (representative_a),
	open_block (open_block_a),
	balance (balance_a),
	modified (modified_a),
	block_count (block_count_a),
	epoch_m (epoch_a)
{
}

bool nano::account_info::deserialize (nano::stream & stream_a)
{
	auto error (false);
	try
	{
		nano::read (stream_a, head.bytes);
		nano::read (stream_a, representative.bytes);
		nano::read (stream_a, open_block.bytes);
		nano::read (stream_a, balance.bytes);
		nano::read (stream_a, modified);
		nano::read (stream_a, block_count);
		nano::read (stream_a, epoch_m);
	}
	catch (std::runtime_error const &)
	{
		error = true;
	}

	return error;
}

bool nano::account_info::operator== (nano::account_info const & other_a) const
{
	return head == other_a.head && representative == other_a.representative && open_block == other_a.open_block && balance == other_a.balance && modified == other_a.modified && block_count == other_a.block_count && epoch () == other_a.epoch ();
}

bool nano::account_info::operator!= (nano::account_info const & other_a) const
{
	return !(*this == other_a);
}

size_t nano::account_info::db_size () const
{
	debug_assert (reinterpret_cast<uint8_t const *> (this) == reinterpret_cast<uint8_t const *> (&head));
	debug_assert (reinterpret_cast<uint8_t const *> (&head) + sizeof (head) == reinterpret_cast<uint8_t const *> (&representative));
	debug_assert (reinterpret_cast<uint8_t const *> (&representative) + sizeof (representative) == reinterpret_cast<uint8_t const *> (&open_block));
	debug_assert (reinterpret_cast<uint8_t const *> (&open_block) + sizeof (open_block) == reinterpret_cast<uint8_t const *> (&balance));
	debug_assert (reinterpret_cast<uint8_t const *> (&balance) + sizeof (balance) == reinterpret_cast<uint8_t const *> (&modified));
	debug_assert (reinterpret_cast<uint8_t const *> (&modified) + sizeof (modified) == reinterpret_cast<uint8_t const *> (&block_count));
	debug_assert (reinterpret_cast<uint8_t const *> (&block_count) + sizeof (block_count) == reinterpret_cast<uint8_t const *> (&epoch_m));
	return sizeof (head) + sizeof (representative) + sizeof (open_block) + sizeof (balance) + sizeof (modified) + sizeof (block_count) + sizeof (epoch_m);
}

nano::epoch nano::account_info::epoch () const
{
	return epoch_m;
}

nano::pending_info::pending_info (nano::account const & source_a, nano::amount const & amount_a, nano::epoch epoch_a) :
	source (source_a),
	amount (amount_a),
	epoch (epoch_a)
{
}

bool nano::pending_info::deserialize (nano::stream & stream_a)
{
	auto error (false);
	try
	{
		nano::read (stream_a, source.bytes);
		nano::read (stream_a, amount.bytes);
		nano::read (stream_a, epoch);
	}
	catch (std::runtime_error const &)
	{
		error = true;
	}

	return error;
}

size_t nano::pending_info::db_size () const
{
	return sizeof (source) + sizeof (amount) + sizeof (epoch);
}

bool nano::pending_info::operator== (nano::pending_info const & other_a) const
{
	return source == other_a.source && amount == other_a.amount && epoch == other_a.epoch;
}

nano::pending_key::pending_key (nano::account const & account_a, nano::block_hash const & hash_a) :
	account (account_a),
	hash (hash_a)
{
}

bool nano::pending_key::deserialize (nano::stream & stream_a)
{
	auto error (false);
	try
	{
		nano::read (stream_a, account.bytes);
		nano::read (stream_a, hash.bytes);
	}
	catch (std::runtime_error const &)
	{
		error = true;
	}

	return error;
}

bool nano::pending_key::operator== (nano::pending_key const & other_a) const
{
	return account == other_a.account && hash == other_a.hash;
}

nano::account const & nano::pending_key::key () const
{
	return account;
}

nano::unchecked_info::unchecked_info (std::shared_ptr<nano::block> const & block_a, nano::account const & account_a, uint64_t modified_a, nano::signature_verification verified_a, bool confirmed_a) :
	block (block_a),
	account (account_a),
	modified (modified_a),
	verified (verified_a),
	confirmed (confirmed_a)
{
}

void nano::unchecked_info::serialize (nano::stream & stream_a) const
{
	debug_assert (block != nullptr);
	nano::serialize_block (stream_a, *block);
	nano::write (stream_a, account.bytes);
	nano::write (stream_a, modified);
	nano::write (stream_a, verified);
}

bool nano::unchecked_info::deserialize (nano::stream & stream_a)
{
	block = nano::deserialize_block (stream_a);
	bool error (block == nullptr);
	if (!error)
	{
		try
		{
			nano::read (stream_a, account.bytes);
			nano::read (stream_a, modified);
			nano::read (stream_a, verified);
		}
		catch (std::runtime_error const &)
		{
			error = true;
		}
	}
	return error;
}

nano::endpoint_key::endpoint_key (std::array<uint8_t, 16> const & address_a, uint16_t port_a) :
	address (address_a), network_port (boost::endian::native_to_big (port_a))
{
}

std::array<uint8_t, 16> const & nano::endpoint_key::address_bytes () const
{
	return address;
}

uint16_t nano::endpoint_key::port () const
{
	return boost::endian::big_to_native (network_port);
}

nano::confirmation_height_info::confirmation_height_info (uint64_t confirmation_height_a, nano::block_hash const & confirmed_frontier_a) :
	height (confirmation_height_a),
	frontier (confirmed_frontier_a)
{
}

void nano::confirmation_height_info::serialize (nano::stream & stream_a) const
{
	nano::write (stream_a, height);
	nano::write (stream_a, frontier);
}

bool nano::confirmation_height_info::deserialize (nano::stream & stream_a)
{
	auto error (false);
	try
	{
		nano::read (stream_a, height);
		nano::read (stream_a, frontier);
	}
	catch (std::runtime_error const &)
	{
		error = true;
	}
	return error;
}

nano::block_info::block_info (nano::account const & account_a, nano::amount const & balance_a) :
	account (account_a),
	balance (balance_a)
{
}

bool nano::vote::operator== (nano::vote const & other_a) const
{
	auto blocks_equal (true);
	if (blocks.size () != other_a.blocks.size ())
	{
		blocks_equal = false;
	}
	else
	{
		for (auto i (0); blocks_equal && i < blocks.size (); ++i)
		{
			auto block (blocks[i]);
			auto other_block (other_a.blocks[i]);
			if (block.which () != other_block.which ())
			{
				blocks_equal = false;
			}
			else if (block.which ())
			{
				if (boost::get<nano::block_hash> (block) != boost::get<nano::block_hash> (other_block))
				{
					blocks_equal = false;
				}
			}
			else
			{
				if (!(*boost::get<std::shared_ptr<nano::block>> (block) == *boost::get<std::shared_ptr<nano::block>> (other_block)))
				{
					blocks_equal = false;
				}
			}
		}
	}
	return timestamp_m == other_a.timestamp_m && blocks_equal && account == other_a.account && signature == other_a.signature;
}

bool nano::vote::operator!= (nano::vote const & other_a) const
{
	return !(*this == other_a);
}

void nano::vote::serialize_json (boost::property_tree::ptree & tree) const
{
	tree.put ("account", account.to_account ());
	tree.put ("signature", signature.number ());
	tree.put ("sequence", std::to_string (timestamp ()));
	tree.put ("timestamp", std::to_string (timestamp ()));
	tree.put ("duration", std::to_string (duration_bits ()));
	boost::property_tree::ptree blocks_tree;
	for (auto block : blocks)
	{
		boost::property_tree::ptree entry;
		if (block.which ())
		{
			entry.put ("", boost::get<nano::block_hash> (block).to_string ());
		}
		else
		{
			entry.put ("", boost::get<std::shared_ptr<nano::block>> (block)->hash ().to_string ());
		}
		blocks_tree.push_back (std::make_pair ("", entry));
	}
	tree.add_child ("blocks", blocks_tree);
}

std::string nano::vote::to_json () const
{
	std::stringstream stream;
	boost::property_tree::ptree tree;
	serialize_json (tree);
	boost::property_tree::write_json (stream, tree);
	return stream.str ();
}

uint64_t nano::vote::timestamp () const
{
	return timestamp_m;
}

uint8_t nano::vote::duration_bits () const
{
	// Duration field is specified in the 4 low-order bits of the timestamp.
	// This makes the timestamp have a minimum granularity of 16ms
	// The duration is specified as 2^(duration + 4) giving it a range of 16-524,288ms in power of two increments
	auto result = timestamp_m & ~timestamp_mask;
	debug_assert (result < 16);
	return static_cast<uint8_t> (result);
}

std::chrono::milliseconds nano::vote::duration () const
{
	return std::chrono::milliseconds{ 1u << (duration_bits () + 4) };
}

nano::vote::vote (nano::vote const & other_a) :
	timestamp_m{ other_a.timestamp_m },
	blocks (other_a.blocks),
	account (other_a.account),
	signature (other_a.signature)
{
}

nano::vote::vote (bool & error_a, nano::stream & stream_a, nano::block_uniquer * uniquer_a)
{
	error_a = deserialize (stream_a, uniquer_a);
}

nano::vote::vote (bool & error_a, nano::stream & stream_a, nano::block_type type_a, nano::block_uniquer * uniquer_a)
{
	try
	{
		nano::read (stream_a, account.bytes);
		nano::read (stream_a, signature.bytes);
		nano::read (stream_a, timestamp_m);

		while (stream_a.in_avail () > 0)
		{
			if (type_a == nano::block_type::not_a_block)
			{
				nano::block_hash block_hash;
				nano::read (stream_a, block_hash);
				blocks.push_back (block_hash);
			}
			else
			{
				auto block (nano::deserialize_block (stream_a, type_a, uniquer_a));
				if (block == nullptr)
				{
					throw std::runtime_error ("Block is null");
				}
				blocks.push_back (block);
			}
		}
	}
	catch (std::runtime_error const &)
	{
		error_a = true;
	}

	if (blocks.empty ())
	{
		error_a = true;
	}
}

nano::vote::vote (nano::account const & account_a, nano::raw_key const & prv_a, uint64_t timestamp_a, uint8_t duration, std::shared_ptr<nano::block> const & block_a) :
	timestamp_m{ packed_timestamp (timestamp_a, duration) },
	blocks (1, block_a),
	account (account_a),
	signature (nano::sign_message (prv_a, account_a, hash ()))
{
}

nano::vote::vote (nano::account const & account_a, nano::raw_key const & prv_a, uint64_t timestamp_a, uint8_t duration, std::vector<nano::block_hash> const & blocks_a) :
	timestamp_m{ packed_timestamp (timestamp_a, duration) },
	account (account_a)
{
	debug_assert (!blocks_a.empty ());
	debug_assert (blocks_a.size () <= 12);
	blocks.reserve (blocks_a.size ());
	std::copy (blocks_a.cbegin (), blocks_a.cend (), std::back_inserter (blocks));
	signature = nano::sign_message (prv_a, account_a, hash ());
}

std::string nano::vote::hashes_string () const
{
	std::string result;
	for (auto hash : *this)
	{
		result += hash.to_string ();
		result += ", ";
	}
	return result;
}

std::string const nano::vote::hash_prefix = "vote ";

nano::block_hash nano::vote::hash () const
{
	nano::block_hash result;
	blake2b_state hash;
	blake2b_init (&hash, sizeof (result.bytes));
	if (blocks.size () > 1 || (!blocks.empty () && blocks.front ().which ()))
	{
		blake2b_update (&hash, hash_prefix.data (), hash_prefix.size ());
	}
	for (auto block_hash : *this)
	{
		blake2b_update (&hash, block_hash.bytes.data (), sizeof (block_hash.bytes));
	}
	union
	{
		uint64_t qword;
		std::array<uint8_t, 8> bytes;
	};
	qword = timestamp_m;
	blake2b_update (&hash, bytes.data (), sizeof (bytes));
	blake2b_final (&hash, result.bytes.data (), sizeof (result.bytes));
	return result;
}

nano::block_hash nano::vote::full_hash () const
{
	nano::block_hash result;
	blake2b_state state;
	blake2b_init (&state, sizeof (result.bytes));
	blake2b_update (&state, hash ().bytes.data (), sizeof (hash ().bytes));
	blake2b_update (&state, account.bytes.data (), sizeof (account.bytes.data ()));
	blake2b_update (&state, signature.bytes.data (), sizeof (signature.bytes.data ()));
	blake2b_final (&state, result.bytes.data (), sizeof (result.bytes));
	return result;
}

void nano::vote::serialize (nano::stream & stream_a, nano::block_type type) const
{
	write (stream_a, account);
	write (stream_a, signature);
	write (stream_a, boost::endian::native_to_little (timestamp_m));
	for (auto const & block : blocks)
	{
		if (block.which ())
		{
			debug_assert (type == nano::block_type::not_a_block);
			write (stream_a, boost::get<nano::block_hash> (block));
		}
		else
		{
			if (type == nano::block_type::not_a_block)
			{
				write (stream_a, boost::get<std::shared_ptr<nano::block>> (block)->hash ());
			}
			else
			{
				boost::get<std::shared_ptr<nano::block>> (block)->serialize (stream_a);
			}
		}
	}
}

void nano::vote::serialize (nano::stream & stream_a) const
{
	write (stream_a, account);
	write (stream_a, signature);
	write (stream_a, boost::endian::native_to_little (timestamp_m));
	for (auto const & block : blocks)
	{
		if (block.which ())
		{
			write (stream_a, nano::block_type::not_a_block);
			write (stream_a, boost::get<nano::block_hash> (block));
		}
		else
		{
			nano::serialize_block (stream_a, *boost::get<std::shared_ptr<nano::block>> (block));
		}
	}
}

bool nano::vote::deserialize (nano::stream & stream_a, nano::block_uniquer * uniquer_a)
{
	auto error (false);
	try
	{
		nano::read (stream_a, account);
		nano::read (stream_a, signature);
		nano::read (stream_a, timestamp_m);
		boost::endian::little_to_native_inplace (timestamp_m);

		nano::block_type type;

		while (true)
		{
			if (nano::try_read (stream_a, type))
			{
				// Reached the end of the stream
				break;
			}

			if (type == nano::block_type::not_a_block)
			{
				nano::block_hash block_hash;
				nano::read (stream_a, block_hash);
				blocks.push_back (block_hash);
			}
			else
			{
				auto block (nano::deserialize_block (stream_a, type, uniquer_a));
				if (block == nullptr)
				{
					throw std::runtime_error ("Block is empty");
				}

				blocks.push_back (block);
			}
		}
	}
	catch (std::runtime_error const &)
	{
		error = true;
	}

	if (blocks.empty ())
	{
		error = true;
	}

	return error;
}

bool nano::vote::validate () const
{
	return nano::validate_message (account, hash (), signature);
}

uint64_t nano::vote::packed_timestamp (uint64_t timestamp, uint8_t duration) const
{
	debug_assert (duration <= duration_max && "Invalid duration");
	debug_assert ((!(timestamp == timestamp_max) || (duration == duration_max)) && "Invalid final vote");
	return (timestamp & timestamp_mask) | duration;
}

nano::block_hash nano::iterate_vote_blocks_as_hash::operator() (boost::variant<std::shared_ptr<nano::block>, nano::block_hash> const & item) const
{
	nano::block_hash result;
	if (item.which ())
	{
		result = boost::get<nano::block_hash> (item);
	}
	else
	{
		result = boost::get<std::shared_ptr<nano::block>> (item)->hash ();
	}
	return result;
}

boost::transform_iterator<nano::iterate_vote_blocks_as_hash, nano::vote_blocks_vec_iter> nano::vote::begin () const
{
	return boost::transform_iterator<nano::iterate_vote_blocks_as_hash, nano::vote_blocks_vec_iter> (blocks.begin (), nano::iterate_vote_blocks_as_hash ());
}

boost::transform_iterator<nano::iterate_vote_blocks_as_hash, nano::vote_blocks_vec_iter> nano::vote::end () const
{
	return boost::transform_iterator<nano::iterate_vote_blocks_as_hash, nano::vote_blocks_vec_iter> (blocks.end (), nano::iterate_vote_blocks_as_hash ());
}

nano::vote_uniquer::vote_uniquer (nano::block_uniquer & uniquer_a) :
	uniquer (uniquer_a)
{
}

std::shared_ptr<nano::vote> nano::vote_uniquer::unique (std::shared_ptr<nano::vote> const & vote_a)
{
	auto result (vote_a);
	if (result != nullptr && !result->blocks.empty ())
	{
		if (!result->blocks.front ().which ())
		{
			result->blocks.front () = uniquer.unique (boost::get<std::shared_ptr<nano::block>> (result->blocks.front ()));
		}
		nano::block_hash key (vote_a->full_hash ());
		nano::lock_guard<nano::mutex> lock (mutex);
		auto & existing (votes[key]);
		if (auto block_l = existing.lock ())
		{
			result = block_l;
		}
		else
		{
			existing = vote_a;
		}

		release_assert (std::numeric_limits<CryptoPP::word32>::max () > votes.size ());
		for (auto i (0); i < cleanup_count && !votes.empty (); ++i)
		{
			auto random_offset = nano::random_pool::generate_word32 (0, static_cast<CryptoPP::word32> (votes.size () - 1));

			auto existing (std::next (votes.begin (), random_offset));
			if (existing == votes.end ())
			{
				existing = votes.begin ();
			}
			if (existing != votes.end ())
			{
				if (auto block_l = existing->second.lock ())
				{
					// Still live
				}
				else
				{
					votes.erase (existing);
				}
			}
		}
	}
	return result;
}

size_t nano::vote_uniquer::size ()
{
	nano::lock_guard<nano::mutex> lock (mutex);
	return votes.size ();
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (vote_uniquer & vote_uniquer, std::string const & name)
{
	auto count = vote_uniquer.size ();
	auto sizeof_element = sizeof (vote_uniquer::value_type);
	auto composite = std::make_unique<container_info_composite> (name);
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "votes", count, sizeof_element }));
	return composite;
}

nano::wallet_id nano::random_wallet_id ()
{
	nano::wallet_id wallet_id;
	nano::uint256_union dummy_secret;
	random_pool::generate_block (dummy_secret.bytes.data (), dummy_secret.bytes.size ());
	ed25519_publickey (dummy_secret.bytes.data (), wallet_id.bytes.data ());
	return wallet_id;
}

nano::unchecked_key::unchecked_key (nano::hash_or_account const & previous_a, nano::block_hash const & hash_a) :
	previous (previous_a.hash),
	hash (hash_a)
{
}

nano::unchecked_key::unchecked_key (nano::uint512_union const & union_a) :
	previous (union_a.uint256s[0].number ()),
	hash (union_a.uint256s[1].number ())
{
}

bool nano::unchecked_key::deserialize (nano::stream & stream_a)
{
	auto error (false);
	try
	{
		nano::read (stream_a, previous.bytes);
		nano::read (stream_a, hash.bytes);
	}
	catch (std::runtime_error const &)
	{
		error = true;
	}

	return error;
}

bool nano::unchecked_key::operator== (nano::unchecked_key const & other_a) const
{
	return previous == other_a.previous && hash == other_a.hash;
}

nano::block_hash const & nano::unchecked_key::key () const
{
	return previous;
}

void nano::generate_cache::enable_all ()
{
	reps = true;
	cemented_count = true;
	unchecked_count = true;
	account_count = true;
}
