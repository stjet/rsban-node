#pragma once

#include <nano/lib/blockbuilders.hpp>
#include <nano/lib/config.hpp>
#include <nano/lib/epoch.hpp>
#include <nano/lib/numbers.hpp>
#include <nano/lib/rsnano.hpp>
#include <nano/lib/stats.hpp>
#include <nano/lib/stream.hpp>
#include <nano/lib/timer.hpp>
#include <nano/lib/utility.hpp>

#include <boost/iterator/transform_iterator.hpp>
#include <boost/optional/optional.hpp>
#include <boost/property_tree/ptree_fwd.hpp>
#include <boost/variant/variant.hpp>

#include <array>

namespace rsnano
{
class VoteHandle;
}

namespace boost
{
template <>
struct hash<::nano::uint256_union>
{
	size_t operator() (::nano::uint256_union const & value_a) const
	{
		return std::hash<::nano::uint256_union> () (value_a);
	}
};

template <>
struct hash<::nano::block_hash>
{
	size_t operator() (::nano::block_hash const & value_a) const
	{
		return std::hash<::nano::block_hash> () (value_a);
	}
};

template <>
struct hash<::nano::hash_or_account>
{
	size_t operator() (::nano::hash_or_account const & data_a) const
	{
		return std::hash<::nano::hash_or_account> () (data_a);
	}
};

template <>
struct hash<::nano::public_key>
{
	size_t operator() (::nano::public_key const & value_a) const
	{
		return std::hash<::nano::public_key> () (value_a);
	}
};
template <>
struct hash<::nano::uint512_union>
{
	size_t operator() (::nano::uint512_union const & value_a) const
	{
		return std::hash<::nano::uint512_union> () (value_a);
	}
};
template <>
struct hash<::nano::qualified_root>
{
	size_t operator() (::nano::qualified_root const & value_a) const
	{
		return std::hash<::nano::qualified_root> () (value_a);
	}
};
}
namespace nano
{
/**
 * A key pair. The private key is generated from the random pool, or passed in
 * as a hex string. The public key is derived using ed25519.
 */
class keypair
{
public:
	keypair ();
	explicit keypair (std::string const &);
	explicit keypair (nano::raw_key &&);
	keypair (nano::keypair const &);
	keypair (nano::raw_key const & priv_key_a, nano::public_key const & pub_key_a);
	nano::public_key pub;
	nano::raw_key prv;
};

class endpoint_key final
{
public:
	endpoint_key () = default;

	/*
	 * @param address_a This should be in network byte order
	 * @param port_a This should be in host byte order
	 */
	endpoint_key (std::array<uint8_t, 16> const & address_a, uint16_t port_a);

	/*
	 * @return The ipv6 address in network byte order
	 */
	std::array<uint8_t, 16> const & address_bytes () const;

	/*
	 * @return The port in host byte order
	 */
	uint16_t port () const;

private:
	// Both stored internally in network byte order
	std::array<uint8_t, 16> address;
	uint16_t network_port{ 0 };
};

enum class no_value
{
	dummy
};

class unchecked_key final
{
public:
	unchecked_key () = default;
	explicit unchecked_key (nano::hash_or_account const & dependency);
	unchecked_key (nano::hash_or_account const &, nano::block_hash const &);
	unchecked_key (nano::uint512_union const &);
	unchecked_key (const rsnano::UncheckedKeyDto & dto);
	rsnano::UncheckedKeyDto to_dto () const;
	bool deserialize (nano::stream &);
	bool operator== (nano::unchecked_key const &) const;
	bool operator< (nano::unchecked_key const &) const;
	nano::block_hash const & key () const;
	nano::block_hash previous{ 0 };
	nano::block_hash hash{ 0 };
};

/**
 * Information on an unchecked block
 */
class unchecked_info final
{
public:
	unchecked_info ();
	unchecked_info (std::shared_ptr<nano::block> const &);
	unchecked_info (nano::unchecked_info const &);
	unchecked_info (nano::unchecked_info &&);
	unchecked_info (rsnano::UncheckedInfoHandle * handle_a);
	~unchecked_info ();
	nano::unchecked_info & operator= (const nano::unchecked_info &);
	void serialize (nano::stream &) const;
	bool deserialize (nano::stream &);
	nano::seconds_t modified () const;
	std::shared_ptr<nano::block> get_block () const;
	rsnano::UncheckedInfoHandle * handle;
};

class block_info final
{
public:
	block_info () = default;
	block_info (nano::account const &, nano::amount const &);
	nano::account account{};
	nano::amount balance{ 0 };
};

class confirmation_height_info final
{
public:
	confirmation_height_info ();
	confirmation_height_info (rsnano::ConfirmationHeightInfoDto dto_a);
	confirmation_height_info (uint64_t, nano::block_hash const &);

	void serialize (nano::stream &) const;
	bool deserialize (nano::stream &);

	/** height of the cemented frontier */
	uint64_t height () const;

	/** hash of the highest cemented block, the cemented/confirmed frontier */
	nano::block_hash frontier () const;
	rsnano::ConfirmationHeightInfoDto dto;
};

namespace confirmation_height
{
	/** When the uncemented count (block count - cemented count) is less than this use the unbounded processor */
	uint64_t const unbounded_cutoff{ 16384 };
}

using vote_blocks_vec_iter = std::vector<nano::block_hash>::const_iterator;
class iterate_vote_blocks_as_hash final
{
public:
	iterate_vote_blocks_as_hash () = default;
	nano::block_hash operator() (nano::block_hash const & item) const;
};

class vote final
{
public:
	vote ();
	vote (nano::account const &);
	vote (rsnano::VoteHandle * handle_a);
	vote (nano::vote const &);
	vote (nano::vote &&);
	vote (bool &, nano::stream &);
	vote (nano::account const &, nano::raw_key const &, nano::millis_t timestamp, uint8_t duration, std::vector<nano::block_hash> const &);
	~vote ();
	std::string hashes_string () const;
	nano::block_hash hash () const;
	nano::block_hash full_hash () const;
	bool operator== (nano::vote const &) const;
	bool operator!= (nano::vote const &) const;
	void serialize (nano::stream &) const;
	/**
	 * Deserializes a vote from the bytes in `stream'
	 * Returns true if there was an error
	 */
	bool deserialize (nano::stream &);
	bool validate () const;
	uint64_t timestamp () const;
	uint8_t duration_bits () const;
	nano::account account () const;
	nano::signature signature () const;
	std::chrono::milliseconds duration () const;
	std::vector<nano::block_hash> hashes () const;
	void flip_signature_bit_0 ();
	rsnano::VoteHandle * get_handle () const;
	// gets the pointer to the block data within Rust;
	const void * get_rust_data_pointer () const;
	static nano::seconds_t constexpr timestamp_max = { 0xffff'ffff'ffff'fff0ULL };
	static uint64_t constexpr timestamp_min = { 0x0000'0000'0000'0010ULL };
	static uint8_t constexpr duration_max = { 0x0fu };

private:
	rsnano::VoteHandle * handle{ nullptr };

public:
	static std::string const hash_prefix;
};

enum class vote_code
{
	invalid, // Vote is not signed correctly
	replay, // Vote does not have the highest timestamp, it's a replay
	vote, // Vote has the highest timestamp
	indeterminate, // Unknown if replay or vote
	ignored, // Vote is valid, but got ingored (e.g. due to cooldown)
};

enum class vote_source
{
	live,
	rebroadcast,
	cache,
};

enum class block_status
{
	progress, // Hasn't been seen before, signed correctly
	bad_signature, // Signature was bad, forged or transmission error
	old, // Already seen and was valid
	negative_spend, // Malicious attempt to spend a negative amount
	fork, // Malicious fork based on previous
	unreceivable, // Source block doesn't exist, has already been received, or requires an account upgrade (epoch blocks)
	gap_previous, // Block marked as previous is unknown
	gap_source, // Block marked as source is unknown
	gap_epoch_open_pending, // Block marked as pending blocks required for epoch open block are unknown
	opened_burn_account, // Block attempts to open the burn account
	balance_mismatch, // Balance and amount delta don't match
	representative_mismatch, // Representative is changed when it is not allowed
	block_position, // This block cannot follow the previous block
	insufficient_work // Insufficient work for this block, even though it passed the minimal validation
};

nano::stat::detail to_stat_detail (block_status);

enum class tally_result
{
	vote,
	changed,
	confirm
};

class network_params;

/** Genesis keys and ledger constants for network variants */
class ledger_constants
{
public:
	ledger_constants () = delete;
	ledger_constants (nano::work_thresholds work, nano::networks network_a);
	ledger_constants (rsnano::LedgerConstantsDto const & dto);
	ledger_constants (ledger_constants const & other_a);
	void read_dto (rsnano::LedgerConstantsDto const & dto);
	nano::work_thresholds work;
	nano::keypair zero_key;
	nano::account nano_beta_account;
	nano::account nano_live_account;
	nano::account nano_test_account;
	std::shared_ptr<nano::block> nano_dev_genesis;
	std::shared_ptr<nano::block> nano_beta_genesis;
	std::shared_ptr<nano::block> nano_live_genesis;
	std::shared_ptr<nano::block> nano_test_genesis;
	std::shared_ptr<nano::block> genesis;
	nano::uint128_t genesis_amount;
	nano::account burn_account;
	nano::epochs epochs;
	rsnano::LedgerConstantsDto to_dto () const;
};

namespace dev
{
	extern nano::keypair genesis_key;
	extern nano::network_params network_params;
	extern nano::ledger_constants & constants;
	extern std::shared_ptr<nano::block> & genesis;
}

/** Constants which depend on random values (always used as singleton) */
class hardened_constants
{
public:
	static hardened_constants & get ();

	nano::account not_an_account;
	nano::uint128_union random_128;

private:
	hardened_constants ();
};

/** Node related constants whose value depends on the active network */
class node_constants
{
public:
	node_constants () = default;
	node_constants (rsnano::NodeConstantsDto const &);
	void read_dto (rsnano::NodeConstantsDto const &);
	rsnano::NodeConstantsDto to_dto () const;
	std::chrono::minutes backup_interval;
	std::chrono::seconds search_pending_interval;
	std::chrono::minutes unchecked_cleaning_interval;
	std::chrono::milliseconds process_confirmed_interval;

	/** The maximum amount of samples for a 2 week period on live or 1 day on beta */
	uint64_t max_weight_samples;
	uint64_t weight_period;
};

/** Voting related constants whose value depends on the active network */
class voting_constants
{
public:
	voting_constants () = default;
	voting_constants (rsnano::VotingConstantsDto const & dto);
	size_t max_cache;
	std::chrono::seconds delay;
	rsnano::VotingConstantsDto to_dto () const;
};

/** Port-mapping related constants whose value depends on the active network */
class portmapping_constants
{
public:
	portmapping_constants () = default;
	portmapping_constants (nano::network_constants & network_constants);
	portmapping_constants (rsnano::PortmappingConstantsDto const & dto);
	// Timeouts are primes so they infrequently happen at the same time
	std::chrono::seconds lease_duration;
	std::chrono::seconds health_check_period;
	rsnano::PortmappingConstantsDto to_dto () const;
};

/** Bootstrap related constants whose value depends on the active network */
class bootstrap_constants
{
public:
	bootstrap_constants () = default;
	bootstrap_constants (rsnano::BootstrapConstantsDto const & dto);
	void read_dto (rsnano::BootstrapConstantsDto const & dto);
	uint32_t lazy_max_pull_blocks;
	uint32_t lazy_min_pull_blocks;
	unsigned frontier_retry_limit;
	unsigned lazy_retry_limit;
	unsigned lazy_destinations_retry_limit;
	std::chrono::milliseconds gap_cache_bootstrap_start_interval;
	uint32_t default_frontiers_age_seconds;
	rsnano::BootstrapConstantsDto to_dto () const;
};

/** Constants whose value depends on the active network */
class network_params
{
public:
	network_params () = delete;
	/** Populate values based on \p network_a */
	network_params (network_params const & other_a);
	network_params (nano::networks network_a);
	network_params (rsnano::NetworkParamsDto const & dto);

	rsnano::NetworkParamsDto to_dto () const;

	unsigned kdf_work;
	nano::work_thresholds work;
	nano::network_constants network;
	nano::ledger_constants ledger;
	nano::voting_constants voting;
	nano::node_constants node;
	nano::portmapping_constants portmapping;
	nano::bootstrap_constants bootstrap;
};

nano::wallet_id random_wallet_id ();
}
