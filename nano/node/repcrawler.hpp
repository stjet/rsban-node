#pragma once

#include "nano/lib/rsnano.hpp"
#include "nano/node/common.hpp"

#include <nano/lib/locks.hpp>
#include <nano/node/transport/channel.hpp>
#include <nano/node/transport/transport.hpp>

#include <boost/circular_buffer.hpp>
#include <boost/multi_index/hashed_index.hpp>
#include <boost/multi_index/mem_fun.hpp>
#include <boost/multi_index/member.hpp>
#include <boost/multi_index/ordered_index.hpp>
#include <boost/multi_index/random_access_index.hpp>
#include <boost/multi_index_container.hpp>
#include <boost/optional.hpp>

#include <chrono>
#include <memory>
#include <thread>

namespace mi = boost::multi_index;

namespace nano
{
class node;
class active_transactions;

/**
 * A representative picked up during repcrawl.
 */
class representative
{
public:
	representative (nano::account account_a, std::shared_ptr<nano::transport::channel> const & channel_a);
	representative (representative const & other_a);
	representative (rsnano::RepresentativeHandle * handle_a);
	~representative ();
	representative & operator= (representative const & other_a);
	size_t channel_id () const
	{
		return get_channel ()->channel_id ();
	}
	bool operator== (nano::representative const & other_a) const
	{
		return get_account () == other_a.get_account ();
	}
	nano::account get_account () const;

	std::shared_ptr<nano::transport::channel> get_channel () const;
	void set_channel (std::shared_ptr<nano::transport::channel> new_channel);

	rsnano::RepresentativeHandle * handle;
};

class rep_crawler_config final
{
public:
	explicit rep_crawler_config (std::chrono::milliseconds query_timeout_a);
	nano::error deserialize (nano::tomlconfig & toml);

public:
	std::chrono::milliseconds query_timeout;
};

class representative_register
{
public:
	class insert_result
	{
	public:
		bool inserted{ false };
		bool updated{ false };
		nano::tcp_endpoint prev_endpoint{};
	};

	representative_register (nano::node & node_a);
	representative_register (representative_register const &) = delete;
	~representative_register ();

	insert_result update_or_insert (nano::account account_a, std::shared_ptr<nano::transport::channel> const & channel_a);
	/** Query if a peer manages a principle representative */
	bool is_pr (std::shared_ptr<nano::transport::channel> const & target_channel) const;
	/** Get total available weight from representatives */
	nano::uint128_t total_weight () const;

	/** Request a list of the top \p count known representatives in descending order of weight, with at least \p mininum_weight voting weight, and optionally with a minimum version \p minimum_protocol_version
	 */
	std::vector<nano::representative> representatives (std::size_t count = std::numeric_limits<std::size_t>::max (), nano::uint128_t const minimum_weight = 0, std::optional<decltype (nano::network_constants::protocol_version)> const & minimum_protocol_version = {});

	/** Request a list of the top \p count known principal representatives in descending order of weight, optionally with a minimum version \p minimum_protocol_version
	 */
	std::vector<representative> principal_representatives (std::size_t count = std::numeric_limits<std::size_t>::max (), std::optional<decltype (nano::network_constants::protocol_version)> const & minimum_protocol_version = {});

	/** Total number of representatives */
	std::size_t representative_count ();

	void cleanup_reps ();
	std::optional<std::chrono::milliseconds> last_request_elapsed (std::shared_ptr<nano::transport::channel> const & target_channel) const;
	void on_rep_request (std::shared_ptr<nano::transport::channel> const & target_channel);

	rsnano::RepresentativeRegisterHandle * handle;

private:
	nano::node & node;
};

/**
 * Crawls the network for representatives. Queries are performed by requesting confirmation of a
 * random block and observing the corresponding vote.
 */
class rep_crawler final
{
public:
	rep_crawler (rep_crawler_config const &, nano::node & node_a);
	rep_crawler (rep_crawler const &) = delete;
	~rep_crawler ();

	void start ();
	void stop ();

	/**
	 * Called when a non-replay vote arrives that might be of interest to rep crawler.
	 * @return true, if the vote was of interest and was processed, this indicates that the rep is likely online and voting
	 */
	bool process (std::shared_ptr<nano::vote> const &, std::shared_ptr<nano::transport::channel> const &);

	/** Attempt to determine if the peer manages one or more representative accounts */
	void query (std::vector<std::shared_ptr<nano::transport::channel>> const & target_channels);

	/** Attempt to determine if the peer manages one or more representative accounts */
	void query (std::shared_ptr<nano::transport::channel> const & target_channel);

	/** Query if a peer manages a principle representative */
	bool is_pr (std::shared_ptr<nano::transport::channel> const &) const;

	/** Get total available weight from representatives */
	nano::uint128_t total_weight () const;

	/** Request a list of the top \p count known representatives in descending order of weight, with at least \p weight_a voting weight, and optionally with a minimum version \p minimum_protocol_version
	 */
	std::vector<representative> representatives (std::size_t count = std::numeric_limits<std::size_t>::max (), nano::uint128_t minimum_weight = 0, std::optional<decltype (nano::network_constants::protocol_version)> const & minimum_protocol_version = {});

	/** Request a list of the top \p count known principal representatives in descending order of weight, optionally with a minimum version \p minimum_protocol_version
	 */
	std::vector<representative> principal_representatives (std::size_t count = std::numeric_limits<std::size_t>::max (), std::optional<decltype (nano::network_constants::protocol_version)> const & minimum_protocol_version = {});

	/** Total number of representatives */
	std::size_t representative_count ();

	std::unique_ptr<container_info_component> collect_container_info (std::string const & name);

private: // Dependencies
	rep_crawler_config const & config;
	nano::node & node;
	nano::stats & stats;
	nano::logger & logger;
	nano::network_constants & network_constants;
	nano::active_transactions & active;

private:
	void run ();
	void cleanup ();
	void validate_and_process (nano::unique_lock<nano::mutex> &);
	bool query_predicate (bool sufficient_weight) const;
	std::chrono::milliseconds query_interval (bool sufficient_weight) const;

	using hash_root_t = std::pair<nano::block_hash, nano::root>;

	/** Returns a list of endpoints to crawl. The total weight is passed in to avoid computing it twice. */
	std::vector<std::shared_ptr<nano::transport::channel>> prepare_crawl_targets (bool sufficient_weight) const;
	std::optional<hash_root_t> prepare_query_target ();
	bool track_rep_request (hash_root_t hash_root, std::shared_ptr<nano::transport::channel> const & channel);

private:
	/**
	 * A representative picked up during repcrawl.
	 */
	struct rep_entry
	{
		rep_entry (nano::account account_a, std::shared_ptr<nano::transport::channel> const & channel_a) :
			account{ account_a },
			channel{ channel_a }
		{
			debug_assert (channel != nullptr);
		}

		nano::account const account;
		std::shared_ptr<nano::transport::channel> channel;

		std::chrono::steady_clock::time_point last_request{};
		std::chrono::steady_clock::time_point last_response{ std::chrono::steady_clock::now () };

		nano::account get_account () const
		{
			return account;
		}
	};

	struct query_entry
	{
		nano::block_hash hash;
		std::shared_ptr<nano::transport::channel> channel;
		std::size_t channel_id;
		std::chrono::steady_clock::time_point time{ std::chrono::steady_clock::now () };
		unsigned int replies{ 0 }; // number of replies to the query
	};

	// clang-format off
	class tag_hash {};
	class tag_channel {};
	class tag_sequenced {};

	using ordered_queries = boost::multi_index_container<query_entry,
	mi::indexed_by<
		mi::hashed_non_unique<mi::tag<tag_channel>,
			mi::member<query_entry, std::size_t, &query_entry::channel_id>>,
		mi::sequenced<mi::tag<tag_sequenced>>,
		mi::hashed_non_unique<mi::tag<tag_hash>,
			mi::member<query_entry, nano::block_hash, &query_entry::hash>>
	>>;
	// clang-format on

	ordered_queries queries;

private:
	static size_t constexpr max_responses{ 1024 * 4 };
	using response_t = std::pair<std::shared_ptr<nano::transport::channel>, std::shared_ptr<nano::vote>>;
	boost::circular_buffer<response_t> responses{ max_responses };

	std::chrono::steady_clock::time_point last_query{};

	std::atomic<bool> stopped{ false };
	nano::condition_variable condition;
	mutable nano::mutex mutex;
	std::thread thread;

public: // Testing
	void force_add_rep (nano::account const & account, std::shared_ptr<nano::transport::channel> const & channel);
	void force_process (std::shared_ptr<nano::vote> const & vote, std::shared_ptr<nano::transport::channel> const & channel);
	void force_query (nano::block_hash const & hash, std::shared_ptr<nano::transport::channel> const & channel);
};
}
