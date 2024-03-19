#include "nano/lib/logging.hpp"
#include "nano/lib/rsnano.hpp"
#include "nano/lib/rsnanoutils.hpp"
#include "nano/node/transport/tcp.hpp"

#include <nano/node/node.hpp>
#include <nano/node/repcrawler.hpp>

#include <boost/format.hpp>

#include <chrono>
#include <memory>
#include <stdexcept>

nano::representative::representative (nano::account account_a, std::shared_ptr<nano::transport::channel> const & channel_a) :
	handle{ rsnano::rsn_representative_create (account_a.bytes.data (), channel_a->handle) }
{
}

nano::representative::representative (rsnano::RepresentativeHandle * handle_a) :
	handle{ handle_a }
{
}

nano::representative::representative (representative const & other_a) :
	handle{ rsnano::rsn_representative_clone (other_a.handle) }
{
}

nano::representative::~representative ()
{
	rsnano::rsn_representative_destroy (handle);
}

nano::representative & nano::representative::operator= (nano::representative const & other_a)
{
	rsnano::rsn_representative_destroy (handle);
	handle = rsnano::rsn_representative_clone (other_a.handle);
	return *this;
}

nano::account nano::representative::get_account () const
{
	nano::account account;
	rsnano::rsn_representative_account (handle, account.bytes.data ());
	return account;
}

std::shared_ptr<nano::transport::channel> nano::representative::get_channel () const
{
	return nano::transport::channel_handle_to_channel (rsnano::rsn_representative_channel (handle));
}

void nano::representative::set_channel (std::shared_ptr<nano::transport::channel> new_channel)
{
	rsnano::rsn_representative_set_channel (handle, new_channel->handle);
}

//------------------------------------------------------------------------------
// representative_register
//------------------------------------------------------------------------------

nano::representative_register::representative_register (nano::node & node_a) :
	node{ node_a }
{
	auto network_dto{ node_a.config->network_params.network.to_dto () };
	handle = rsnano::rsn_representative_register_create (
	node_a.ledger.handle,
	node_a.online_reps.get_handle (),
	node_a.stats->handle,
	&network_dto);
}

nano::representative_register::~representative_register ()
{
	rsnano::rsn_representative_register_destroy (handle);
}

nano::representative_register::insert_result nano::representative_register::update_or_insert (nano::account account_a, std::shared_ptr<nano::transport::channel> const & channel_a)
{
	rsnano::EndpointDto endpoint_dto;
	auto result_code = rsnano::rsn_representative_register_update_or_insert (handle, account_a.bytes.data (), channel_a->handle, &endpoint_dto);
	nano::representative_register::insert_result result{};
	if (result_code == 0)
	{
		result.inserted = true;
	}
	else if (result_code == 1)
	{
		// updated
	}
	else if (result_code == 2)
	{
		result.updated = true;
		result.prev_endpoint = rsnano::dto_to_endpoint (endpoint_dto);
	}
	else
	{
		throw std::runtime_error ("unknown result code");
	}
	return result;
}

bool nano::representative_register::is_pr (std::shared_ptr<nano::transport::channel> const & target_channel) const
{
	return rsnano::rsn_representative_register_is_pr (handle, target_channel->handle);
}

nano::uint128_t nano::representative_register::total_weight () const
{
	nano::amount result;
	rsnano::rsn_representative_register_total_weight (handle, result.bytes.data ());
	return result.number ();
}

std::vector<nano::representative> nano::representative_register::representatives (std::size_t count, nano::uint128_t const minimum_weight, std::optional<decltype (nano::network_constants::protocol_version)> const & minimum_protocol_version)
{
	uint8_t min_version = minimum_protocol_version.value_or (0);
	nano::amount weight{ minimum_weight };

	auto result_handle = rsnano::rsn_representative_register_representatives (handle, count, weight.bytes.data (), min_version);

	auto len = rsnano::rsn_representative_list_len (result_handle);
	std::vector<nano::representative> result;
	result.reserve (len);
	for (auto i = 0; i < len; ++i)
	{
		result.emplace_back (rsnano::rsn_representative_list_get (result_handle, i));
	}
	rsnano::rsn_representative_list_destroy (result_handle);
	return result;
}

std::vector<nano::representative> nano::representative_register::principal_representatives (std::size_t count, std::optional<decltype (nano::network_constants::protocol_version)> const & minimum_protocol_version)
{
	return representatives (count, node.minimum_principal_weight (), minimum_protocol_version);
}

/** Total number of representatives */
std::size_t nano::representative_register::representative_count ()
{
	return rsnano::rsn_representative_register_count (handle);
}

void nano::representative_register::cleanup_reps ()
{
	rsnano::rsn_representative_register_cleanup_reps (handle);
}

std::optional<std::chrono::milliseconds> nano::representative_register::last_request_elapsed (std::shared_ptr<nano::transport::channel> const & target_channel) const
{
	auto elapsed_ms = rsnano::rsn_representative_register_last_request_elapsed_ms (handle, target_channel->handle);
	if (elapsed_ms < 0)
	{
		return {};
	}
	else
	{
		return std::chrono::milliseconds (elapsed_ms);
	}
}

void nano::representative_register::on_rep_request (std::shared_ptr<nano::transport::channel> const & target_channel)
{
	rsnano::rsn_representative_register_on_rep_request (handle, target_channel->handle);
}

nano::rep_crawler::rep_crawler (nano::rep_crawler_config const & config_a, nano::node & node_a) :
	config{ config_a },
	node (node_a),
	stats{ *node_a.stats },
	logger{ *node_a.logger },
	network_constants{ node_a.network_params.network },
	active{ node_a.active }
{
	if (!node.flags.disable_rep_crawler ())
	{
		node.observers->endpoint.add ([this] (std::shared_ptr<nano::transport::channel> const & channel) {
			query (channel);
		});
	}
}

nano::rep_crawler::~rep_crawler ()
{
	// Thread must be stopped before destruction
	debug_assert (!thread.joinable ());
}

void nano::rep_crawler::start ()
{
	debug_assert (!thread.joinable ());

	thread = std::thread{ [this] () {
		nano::thread_role::set (nano::thread_role::name::rep_crawler);
		run ();
	} };
}

void nano::rep_crawler::stop ()
{
	{
		nano::lock_guard<nano::mutex> lock{ mutex };
		stopped = true;
	}
	condition.notify_all ();
	if (thread.joinable ())
	{
		thread.join ();
	}
}

// Exits with the lock unlocked
void nano::rep_crawler::validate_and_process (nano::unique_lock<nano::mutex> & lock)
{
	debug_assert (!mutex.try_lock ());
	debug_assert (lock.owns_lock ());
	debug_assert (!responses.empty ()); // Should be checked before calling this function

	decltype (responses) responses_l{ responses.capacity () };
	responses_l.swap (responses);

	lock.unlock ();

	// normally the rep_crawler only tracks principal reps but it can be made to track
	// reps with less weight by setting rep_crawler_weight_minimum to a low value
	auto const minimum = std::min (node.minimum_principal_weight (), node.config->rep_crawler_weight_minimum.number ());

	// TODO: Is it really faster to repeatedly lock/unlock the mutex for each response?
	for (auto const & response : responses_l)
	{
		auto & vote = response.second;
		auto & channel = response.first;
		release_assert (vote != nullptr);
		release_assert (channel != nullptr);

		if (channel->get_type () == nano::transport::transport_type::loopback)
		{
			logger.debug (nano::log::type::rep_crawler, "Ignoring vote from loopback channel: {}", channel->to_string ());
			continue;
		}

		nano::uint128_t const rep_weight = node.ledger.weight (vote->account ());
		if (rep_weight < minimum)
		{
			logger.debug (nano::log::type::rep_crawler, "Ignoring vote from account {} with too little voting weight: {}",
			vote->account ().to_account (),
			nano::util::to_str (rep_weight));
			continue;
		}

		// temporary data used for logging after dropping the lock
		bool inserted = false;
		bool updated = false;
		std::shared_ptr<nano::transport::channel> prev_channel;

		auto result{ node.representative_register.update_or_insert (vote->account (), channel) };

		if (result.inserted)
		{
			logger.info (nano::log::type::rep_crawler, "Found representative {} at {}", vote->account ().to_account (), channel->to_string ());
		}
		if (result.updated)
		{
			logger.warn (nano::log::type::rep_crawler, "Updated representative {} at {} (was at: {})", vote->account ().to_account (), channel->to_string (), result.prev_endpoint.address ().to_string ());
		}
	}
}

std::chrono::milliseconds nano::rep_crawler::query_interval (bool sufficient_weight) const
{
	return sufficient_weight ? network_constants.rep_crawler_normal_interval : network_constants.rep_crawler_warmup_interval;
}

bool nano::rep_crawler::query_predicate (bool sufficient_weight) const
{
	return nano::elapsed (last_query, query_interval (sufficient_weight));
}

void nano::rep_crawler::run ()
{
	nano::unique_lock<nano::mutex> lock{ mutex };
	while (!stopped)
	{
		lock.unlock ();

		auto const current_total_weight = total_weight ();
		bool const sufficient_weight = current_total_weight > node.online_reps.delta ();

		// If online weight drops below minimum, reach out to preconfigured peers
		if (!sufficient_weight)
		{
			stats.inc (nano::stat::type::rep_crawler, nano::stat::detail::keepalive);
			node.keepalive_preconfigured ();
		}

		lock.lock ();

		condition.wait_for (lock, query_interval (sufficient_weight), [this, sufficient_weight] {
			return stopped || query_predicate (sufficient_weight) || !responses.empty ();
		});

		if (stopped)
		{
			return;
		}

		stats.inc (nano::stat::type::rep_crawler, nano::stat::detail::loop);

		if (!responses.empty ())
		{
			validate_and_process (lock);
			debug_assert (!lock.owns_lock ());
			lock.lock ();
		}

		cleanup ();

		if (query_predicate (sufficient_weight))
		{
			last_query = std::chrono::steady_clock::now ();

			auto targets = prepare_crawl_targets (sufficient_weight);

			lock.unlock ();
			query (targets);
			lock.lock ();
		}

		debug_assert (lock.owns_lock ());
	}
}

void nano::rep_crawler::cleanup ()
{
	debug_assert (!mutex.try_lock ());

	// Evict reps with dead channels
	node.representative_register.cleanup_reps ();

	// Evict queries that haven't been responded to in a while
	erase_if (queries, [this] (query_entry const & query) {
		if (nano::elapsed (query.time, config.query_timeout))
		{
			if (query.replies == 0)
			{
				logger.debug (nano::log::type::rep_crawler, "Aborting unresponsive query for block {} from {}", query.hash.to_string (), query.channel->to_string ());
				stats.inc (nano::stat::type::rep_crawler, nano::stat::detail::query_timeout);
			}
			else
			{
				logger.debug (nano::log::type::rep_crawler, "Completion of query with {} replies for block {} from {}", query.replies, query.hash.to_string (), query.channel->to_string ());
				stats.inc (nano::stat::type::rep_crawler, nano::stat::detail::query_completion);
			}
			return true; // Erase
		}
		return false;
	});
}

std::vector<std::shared_ptr<nano::transport::channel>> nano::rep_crawler::prepare_crawl_targets (bool sufficient_weight) const
{
	debug_assert (!mutex.try_lock ());

	// TODO: Make these values configurable
	constexpr std::size_t conservative_count = 160;
	constexpr std::size_t aggressive_count = 160;
	constexpr std::size_t conservative_max_attempts = 4;
	constexpr std::size_t aggressive_max_attempts = 8;
	std::chrono::milliseconds rep_query_interval = node.network_params.network.is_dev_network () ? std::chrono::milliseconds{ 500 } : std::chrono::milliseconds{ 60 * 1000 };

	stats.inc (nano::stat::type::rep_crawler, sufficient_weight ? nano::stat::detail::crawl_normal : nano::stat::detail::crawl_aggressive);

	// Crawl more aggressively if we lack sufficient total peer weight.
	auto const required_peer_count = sufficient_weight ? conservative_count : aggressive_count;

	auto random_peers = node.network->random_channels (required_peer_count, 0, /* include channels with ephemeral remote ports */ true);

	auto should_query = [&, this] (std::shared_ptr<nano::transport::channel> const & channel) {
		auto last_request_elapsed = node.representative_register.last_request_elapsed (channel);
		if (last_request_elapsed)
		{
			// Throttle queries to active reps
			return last_request_elapsed >= rep_query_interval;
		}
		else
		{
			// Avoid querying the same peer multiple times when rep crawler is warmed up
			auto const max_attempts = sufficient_weight ? conservative_max_attempts : aggressive_max_attempts;
			return queries.get<tag_channel> ().count (channel->channel_id ()) < max_attempts;
		}
	};

	erase_if (random_peers, [&, this] (std::shared_ptr<nano::transport::channel> const & channel) {
		return !should_query (channel);
	});

	return { random_peers.begin (), random_peers.end () };
}

auto nano::rep_crawler::prepare_query_target () -> std::optional<hash_root_t>
{
	constexpr int max_attempts = 4;

	auto transaction = node.store.tx_begin_read ();

	std::optional<std::pair<nano::block_hash, nano::block_hash>> hash_root;

	// Randomly select a block from ledger to request votes for
	for (auto i = 0; i < max_attempts && !hash_root; ++i)
	{
		hash_root = node.ledger.hash_root_random (*transaction);

		// Rebroadcasted votes for recently confirmed blocks might confuse the rep crawler
		if (active.recently_confirmed.exists (hash_root->first))
		{
			hash_root = std::nullopt;
		}
	}

	if (!hash_root)
	{
		return std::nullopt;
	}

	// Don't send same block multiple times in tests
	if (node.network_params.network.is_dev_network ())
	{
		nano::lock_guard<nano::mutex> lock{ mutex };

		for (auto i = 0; queries.get<tag_hash> ().count (hash_root->first) != 0 && i < max_attempts; ++i)
		{
			hash_root = node.ledger.hash_root_random (*transaction);
		}
	}

	return hash_root;
}

bool nano::rep_crawler::track_rep_request (hash_root_t hash_root, std::shared_ptr<nano::transport::channel> const & channel)
{
	debug_assert (!mutex.try_lock ());

	auto [_, inserted] = queries.emplace (query_entry{ hash_root.first, channel, channel->channel_id () });
	if (!inserted)
	{
		return false; // Duplicate, not tracked
	}

	// Find and update the timestamp on all reps available on the endpoint (a single host may have multiple reps)
	node.representative_register.on_rep_request (channel);

	return true;
}

void nano::rep_crawler::query (std::vector<std::shared_ptr<nano::transport::channel>> const & target_channels)
{
	auto maybe_hash_root = prepare_query_target ();
	if (!maybe_hash_root)
	{
		logger.debug (nano::log::type::rep_crawler, "No block to query");
		stats.inc (nano::stat::type::rep_crawler, nano::stat::detail::query_target_failed);
		return;
	}
	auto hash_root = *maybe_hash_root;

	nano::lock_guard<nano::mutex> lock{ mutex };

	for (const auto & channel : target_channels)
	{
		debug_assert (channel != nullptr);

		bool tracked = track_rep_request (hash_root, channel);
		if (tracked)
		{
			logger.debug (nano::log::type::rep_crawler, "Sending query for block {} to {}", hash_root.first.to_string (), channel->to_string ());
			stats.inc (nano::stat::type::rep_crawler, nano::stat::detail::query_sent);

			auto const & [hash, root] = hash_root;
			nano::confirm_req req{ network_constants, hash, root };

			channel->send (
			req,
			[this] (auto & ec, auto size) {
				if (ec)
				{
					stats.inc (nano::stat::type::rep_crawler, nano::stat::detail::write_error, nano::stat::dir::out);
				}
			},
			nano::transport::buffer_drop_policy::no_socket_drop);
		}
		else
		{
			logger.debug (nano::log::type::rep_crawler, "Ignoring duplicate query for block {} to {}", hash_root.first.to_string (), channel->to_string ());
			stats.inc (nano::stat::type::rep_crawler, nano::stat::detail::query_duplicate);
		}
	}
}

void nano::rep_crawler::query (std::shared_ptr<nano::transport::channel> const & target_channel)
{
	query (std::vector{ target_channel });
}

bool nano::rep_crawler::is_pr (std::shared_ptr<nano::transport::channel> const & channel) const
{
	return node.representative_register.is_pr (channel);
}

bool nano::rep_crawler::process (std::shared_ptr<nano::vote> const & vote, std::shared_ptr<nano::transport::channel> const & channel)
{
	nano::lock_guard<nano::mutex> lock{ mutex };

	auto & index = queries.get<tag_channel> ();
	auto [begin, end] = index.equal_range (channel->channel_id ());
	for (auto it = begin; it != end; ++it)
	{
		// TODO: This linear search could be slow, especially with large votes.
		auto const target_hash = it->hash;
		auto hashes{ vote->hashes () };
		bool found = std::any_of (hashes.begin (), hashes.end (), [&target_hash] (nano::block_hash const & hash) {
			return hash == target_hash;
		});

		if (found)
		{
			logger.debug (nano::log::type::rep_crawler, "Processing response for block {} from {}", target_hash.to_string (), channel->to_string ());
			stats.inc (nano::stat::type::rep_crawler, nano::stat::detail::response);
			// TODO: Track query response time

			responses.push_back ({ channel, vote });
			queries.modify (it, [] (query_entry & e) {
				e.replies++;
			});
			condition.notify_all ();
			return true; // Found and processed
		}
	}
	return false;
}

nano::uint128_t nano::rep_crawler::total_weight () const
{
	return node.representative_register.total_weight ();
}

std::vector<nano::representative> nano::rep_crawler::representatives (std::size_t count, nano::uint128_t const minimum_weight, std::optional<decltype (nano::network_constants::protocol_version)> const & minimum_protocol_version)
{
	return node.representative_register.representatives (count, minimum_weight, minimum_protocol_version);
}

std::vector<nano::representative> nano::rep_crawler::principal_representatives (std::size_t count, std::optional<decltype (nano::network_constants::protocol_version)> const & minimum_protocol_version)
{
	return representatives (count, node.minimum_principal_weight (), minimum_protocol_version);
}

std::size_t nano::rep_crawler::representative_count ()
{
	return node.representative_register.representative_count ();
}

std::unique_ptr<nano::container_info_component> nano::rep_crawler::collect_container_info (const std::string & name)
{
	nano::lock_guard<nano::mutex> guard{ mutex };

	auto composite = std::make_unique<container_info_composite> (name);
	auto reps_count = node.representative_register.representative_count ();
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "reps", reps_count, 97 }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "queries", queries.size (), sizeof (decltype (queries)::value_type) }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "responses", responses.size (), sizeof (decltype (responses)::value_type) }));
	return composite;
}

// Only for tests
void nano::rep_crawler::force_add_rep (const nano::account & account, const std::shared_ptr<nano::transport::channel> & channel)
{
	release_assert (node.network_params.network.is_dev_network ());
	node.representative_register.update_or_insert (account, channel);
}

// Only for tests
void nano::rep_crawler::force_process (const std::shared_ptr<nano::vote> & vote, const std::shared_ptr<nano::transport::channel> & channel)
{
	release_assert (node.network_params.network.is_dev_network ());
	nano::lock_guard<nano::mutex> lock{ mutex };
	responses.push_back ({ channel, vote });
}

// Only for tests
void nano::rep_crawler::force_query (const nano::block_hash & hash, const std::shared_ptr<nano::transport::channel> & channel)
{
	release_assert (node.network_params.network.is_dev_network ());
	nano::lock_guard<nano::mutex> lock{ mutex };
	queries.emplace (query_entry{ hash, channel, channel->channel_id () });
}

/*
 * rep_crawler_config
 */

nano::rep_crawler_config::rep_crawler_config (std::chrono::milliseconds query_timeout_a) :
	query_timeout{ query_timeout_a }
{
}

nano::error nano::rep_crawler_config::deserialize (nano::tomlconfig & toml)
{
	auto query_timeout_l = query_timeout.count ();
	toml.get ("query_timeout", query_timeout_l);
	query_timeout = std::chrono::milliseconds{ query_timeout_l };

	return toml.get_error ();
}
