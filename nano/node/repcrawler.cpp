#include "nano/lib/rsnano.hpp"
#include "nano/node/transport/tcp.hpp"

#include <nano/node/node.hpp>
#include <nano/node/repcrawler.hpp>

#include <boost/format.hpp>

#include <chrono>
#include <memory>

nano::representative::representative (nano::account account_a, std::shared_ptr<nano::transport::channel> const & channel_a) :
	handle{ rsnano::rsn_representative_create (account_a.bytes.data (), channel_a->handle) }
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

std::chrono::steady_clock::time_point nano::representative::get_last_request () const
{
	return std::chrono::steady_clock::time_point (
	std::chrono::steady_clock::duration (rsnano::rsn_representative_last_request (handle)));
}

void nano::representative::set_last_request (std::chrono::steady_clock::time_point time_point)
{
	auto timepoint_ns = std::chrono::duration_cast<std::chrono::nanoseconds> (time_point.time_since_epoch ()).count ();
	rsnano::rsn_representative_set_last_request (handle, timepoint_ns);
}

std::chrono::steady_clock::time_point nano::representative::get_last_response () const
{
	return std::chrono::steady_clock::time_point (
	std::chrono::steady_clock::duration (rsnano::rsn_representative_last_response (handle)));
}

void nano::representative::set_last_response (std::chrono::steady_clock::time_point time_point)
{
	rsnano::rsn_representative_set_last_response (handle, time_point.time_since_epoch ().count ());
}

nano::representative_register::representative_register  (nano::node & node_a) : 
	node{node_a}
{
}

void nano::representative_register::insert (nano::account account_a, std::shared_ptr<nano::transport::channel> const & channel_a)
{
	//TODO mutex lock!?
	probable_reps.emplace(account_a, channel_a);
}

void nano::representative_register::update_or_insert (nano::account account_a, std::shared_ptr<nano::transport::channel> const & channel_a)
{
	// temporary data used for logging after dropping the lock
	auto inserted = false;
	auto updated = false;
	std::shared_ptr<nano::transport::channel> prev_channel;
	nano::unique_lock<nano::mutex> lock{ probable_reps_mutex };

	auto existing (probable_reps.find (account_a));
	if (existing != probable_reps.end ())
	{
		probable_reps.modify (existing, [&updated, &account_a, &channel_a, &prev_channel] (nano::representative & info) {
			info.set_last_response (std::chrono::steady_clock::now ());

			auto info_channel = info.get_channel ();
			// Update if representative channel was changed
			if (info_channel->get_remote_endpoint () != channel_a->get_remote_endpoint ())
			{
				debug_assert (info.get_account () == account_a);
				updated = true;
				prev_channel = info_channel;
				info.set_channel (channel_a);
			}
		});
	}
	else
	{
		insert (account_a, channel_a);
		// rsnano::rsn_rep_crawler_add (handle, rep.handle);
		inserted = true;
	}

	lock.unlock ();

	if (inserted)
	{
		node.logger->try_log (boost::str (boost::format ("Found representative %1% at %2%") % account_a.to_account () % channel_a->to_string ()));
	}

	if (updated)
	{
		node.logger->try_log (boost::str (boost::format ("Updated representative %1% at %2% (was at: %3%)") % account_a.to_account () % channel_a->to_string () % prev_channel->to_string ()));
	}
}

nano::rep_crawler::rep_crawler (nano::node & node_a) :
	representative_register{node_a},
	node (node_a),
	handle{ rsnano::rsn_rep_crawler_create () }
{
	if (!node.flags.disable_rep_crawler ())
	{
		node.observers->endpoint.add ([this] (std::shared_ptr<nano::transport::channel> const & channel_a) {
			this->query (channel_a);
		});
	}
}

nano::rep_crawler::~rep_crawler ()
{
	rsnano::rsn_rep_crawler_destroy (handle);
}

void nano::rep_crawler::remove (nano::block_hash const & hash_a)
{
	rsnano::rsn_rep_crawler_remove (handle, hash_a.bytes.data ());
}

void nano::rep_crawler::start ()
{
	ongoing_crawl ();
}

void nano::rep_crawler::validate ()
{
	decltype (responses) responses_l;
	{
		nano::lock_guard<nano::mutex> lock{ active_mutex };
		responses_l.swap (responses);
	}

	// normally the rep_crawler only tracks principal reps but it can be made to track
	// reps with less weight by setting rep_crawler_weight_minimum to a low value
	auto minimum = std::min (node.minimum_principal_weight (), node.config->rep_crawler_weight_minimum.number ());

	for (auto const & i : responses_l)
	{
		auto & vote = i.second;
		auto & channel = i.first;
		debug_assert (channel != nullptr);

		if (channel->get_type () == nano::transport::transport_type::loopback)
		{
			if (node.config->logging.rep_crawler_logging ())
			{
				node.logger->try_log (boost::str (boost::format ("rep_crawler ignoring vote from loopback channel %1%") % channel->to_string ()));
			}
			continue;
		}

		nano::uint128_t rep_weight = node.ledger.weight (vote->account ());
		if (rep_weight < minimum)
		{
			if (node.config->logging.rep_crawler_logging ())
			{
				node.logger->try_log (boost::str (boost::format ("rep_crawler ignoring vote from account %1% with too little voting weight %2%") % vote->account ().to_account () % rep_weight));
			}
			continue;
		}
		//
		//---------------------------
		// new function: insert_or_update() ?

		// temporary data used for logging after dropping the lock
		auto inserted = false;
		auto updated = false;
		std::shared_ptr<nano::transport::channel> prev_channel;

		nano::unique_lock<nano::mutex> lock{ representative_register.probable_reps_mutex };

		auto existing (representative_register.probable_reps.find (vote->account ()));
		if (existing != representative_register.probable_reps.end ())
		{
			representative_register.probable_reps.modify (existing, [rep_weight, &updated, &vote, &channel, &prev_channel] (nano::representative & info) {
				info.set_last_response (std::chrono::steady_clock::now ());

				auto info_channel = info.get_channel ();
				// Update if representative channel was changed
				if (info_channel->get_remote_endpoint () != channel->get_remote_endpoint ())
				{
					debug_assert (info.get_account () == vote->account ());
					updated = true;
					prev_channel = info_channel;
					info.set_channel (channel);
				}
			});
		}
		else
		{
			representative_register.insert (vote->account (), channel);
			// rsnano::rsn_rep_crawler_add (handle, rep.handle);
			inserted = true;
		}

		lock.unlock ();

		if (inserted)
		{
			node.logger->try_log (boost::str (boost::format ("Found representative %1% at %2%") % vote->account ().to_account () % channel->to_string ()));
		}

		if (updated)
		{
			node.logger->try_log (boost::str (boost::format ("Updated representative %1% at %2% (was at: %3%)") % vote->account ().to_account () % channel->to_string () % prev_channel->to_string ()));
		}
		//---------------------------
	}
}

void nano::rep_crawler::ongoing_crawl ()
{
	auto total_weight_l (total_weight ());
	cleanup_reps ();
	validate ();
	query (get_crawl_targets (total_weight_l));
	auto sufficient_weight (total_weight_l > node.online_reps.delta ());
	// If online weight drops below minimum, reach out to preconfigured peers
	if (!sufficient_weight)
	{
		node.keepalive_preconfigured (node.config->preconfigured_peers);
	}
	// Reduce crawl frequency when there's enough total peer weight
	unsigned next_run_ms = node.network_params.network.is_dev_network () ? 100 : sufficient_weight ? 7000
																								   : 3000;
	std::weak_ptr<nano::node> node_w (node.shared ());
	auto now (std::chrono::steady_clock::now ());
	node.workers->add_timed_task (now + std::chrono::milliseconds (next_run_ms), [node_w, this] () {
		if (auto node_l = node_w.lock ())
		{
			this->ongoing_crawl ();
		}
	});
}

std::vector<std::shared_ptr<nano::transport::channel>> nano::rep_crawler::get_crawl_targets (nano::uint128_t total_weight_a)
{
	constexpr std::size_t conservative_count = 10;
	constexpr std::size_t aggressive_count = 40;

	// Crawl more aggressively if we lack sufficient total peer weight.
	bool sufficient_weight (total_weight_a > node.online_reps.delta ());
	uint16_t required_peer_count = sufficient_weight ? conservative_count : aggressive_count;

	// Add random peers. We do this even if we have enough weight, in order to pick up reps
	// that didn't respond when first observed. If the current total weight isn't sufficient, this
	// will be more aggressive. When the node first starts, the rep container is empty and all
	// endpoints will originate from random peers.
	required_peer_count += required_peer_count / 2;

	// The rest of the endpoints are picked randomly
	return node.network->tcp_channels->random_channels (required_peer_count, 0, true); // Include channels with ephemeral remote ports
}

void nano::rep_crawler::query (std::vector<std::shared_ptr<nano::transport::channel>> const & channels_a)
{
	auto transaction (node.store.tx_begin_read ());
	auto hash_root (node.ledger.hash_root_random (*transaction));
	{
		// Don't send same block multiple times in tests
		if (node.network_params.network.is_dev_network ())
		{
			for (auto i (0); rsnano::rsn_rep_crawler_active_contains (handle, hash_root.first.bytes.data ()) && i < 4; ++i)
			{
				hash_root = node.ledger.hash_root_random (*transaction);
			}
		}
		rsnano::rsn_rep_crawler_active_insert (handle, hash_root.first.bytes.data ());
	}
	if (!channels_a.empty ())
	{
		// In case our random block is a recently confirmed one, we remove an entry otherwise votes will be marked as replay and not forwarded to repcrawler
		node.active.recently_confirmed.erase (hash_root.first);
	}
	for (auto i (channels_a.begin ()), n (channels_a.end ()); i != n; ++i)
	{
		debug_assert (*i != nullptr);
		on_rep_request (*i);
		// Confirmation request with hash + root
		nano::confirm_req req (node.network_params.network, hash_root.first, hash_root.second);
		(*i)->send(req);
	}

	// A representative must respond with a vote within the deadline
	std::weak_ptr<nano::node> node_w (node.shared ());
	node.workers->add_timed_task (std::chrono::steady_clock::now () + std::chrono::seconds (5), [node_w, hash = hash_root.first] () {
		if (auto node_l = node_w.lock ())
		{
			auto target_finished_processed (node_l->vote_processor.total_processed + node_l->vote_processor_queue.size ());
			node_l->rep_crawler.throttled_remove (hash, target_finished_processed);
		}
	});
}

void nano::rep_crawler::query (std::shared_ptr<nano::transport::channel> const & channel_a)
{
	std::vector<std::shared_ptr<nano::transport::channel>> peers;
	peers.emplace_back (channel_a);
	query (peers);
}

void nano::rep_crawler::throttled_remove (nano::block_hash const & hash_a, uint64_t const target_finished_processed)
{
	if (node.vote_processor.total_processed >= target_finished_processed)
	{
		remove (hash_a);
	}
	else
	{
		std::weak_ptr<nano::node> node_w (node.shared ());
		node.workers->add_timed_task (std::chrono::steady_clock::now () + std::chrono::seconds (5), [node_w, hash_a, target_finished_processed] () {
			if (auto node_l = node_w.lock ())
			{
				node_l->rep_crawler.throttled_remove (hash_a, target_finished_processed);
			}
		});
	}
}

bool nano::rep_crawler::is_pr (nano::transport::channel const & channel_a) const
{
	nano::lock_guard<nano::mutex> lock{ representative_register.probable_reps_mutex };
	auto existing = representative_register.probable_reps.get<nano::representative_register::tag_channel_id> ().find (channel_a.channel_id ());
	bool result = false;
	if (existing != representative_register.probable_reps.get<nano::representative_register::tag_channel_id> ().end ())
	{
		result = node.ledger.weight (existing->get_account ()) > node.minimum_principal_weight ();
	}
	return result;
}

void nano::rep_crawler::insert_active (nano::block_hash const & hash_a)
{
	rsnano::rsn_rep_crawler_active_insert (handle, hash_a.bytes.data ());
}

void nano::rep_crawler::insert_response (std::shared_ptr<nano::transport::channel> channel_a, std::shared_ptr<nano::vote> vote_a)
{
	rsnano::rsn_rep_crawler_response_insert (handle, channel_a->handle, vote_a->get_handle ());
}

bool nano::rep_crawler::response (std::shared_ptr<nano::transport::channel> const & channel_a, std::shared_ptr<nano::vote> const & vote_a, bool force)
{
	bool error = true;
	nano::lock_guard<nano::mutex> lock{ active_mutex };
	auto hashes = vote_a->hashes ();
	for (auto i = hashes.begin (), n = hashes.end (); i != n; ++i)
	{
		if (force || rsnano::rsn_rep_crawler_active_contains (handle, i->bytes.data ()))
		{
			responses.emplace_back (channel_a, vote_a);
			error = false;
			break;
		}
	}
	return error;
}

nano::uint128_t nano::rep_crawler::total_weight () const
{
	nano::lock_guard<nano::mutex> lock{ representative_register.probable_reps_mutex };
	nano::uint128_t result (0);
	for (auto i (representative_register.probable_reps.get<nano::representative_register::tag_account> ().begin ()), n (representative_register.probable_reps.get<nano::representative_register::tag_account> ().end ()); i != n; ++i)
	{
		if (i->get_channel ()->alive ())
		{
			result += node.ledger.weight (i->get_account ());
		}
	}
	return result;
}

void nano::rep_crawler::on_rep_request (std::shared_ptr<nano::transport::channel> const & channel_a)
{
	nano::lock_guard<nano::mutex> lock{ representative_register.probable_reps_mutex };
	if (channel_a->get_tcp_remote_endpoint ().address () != boost::asio::ip::address_v6::any ())
	{
		nano::representative_register::probably_rep_t::index<nano::representative_register::tag_channel_id>::type & channel_id_index = representative_register.probable_reps.get<nano::representative_register::tag_channel_id> ();

		// Find and update the timestamp on all reps available on the endpoint (a single host may have multiple reps)
		auto itr_pair = channel_id_index.equal_range (channel_a->channel_id ());
		for (; itr_pair.first != itr_pair.second; itr_pair.first++)
		{
			channel_id_index.modify (itr_pair.first, [] (nano::representative & value_a) {
				value_a.set_last_request (std::chrono::steady_clock::now ());
			});
		}
	}
}

void nano::rep_crawler::cleanup_reps ()
{
	// Check known rep channels
	nano::lock_guard<nano::mutex> lock{ representative_register.probable_reps_mutex };
	auto iterator (representative_register.probable_reps.get<nano::representative_register::tag_last_request> ().begin ());
	while (iterator != representative_register.probable_reps.get<nano::representative_register::tag_last_request> ().end ())
	{
		if (iterator->get_channel ()->alive ())
		{
			++iterator;
		}
		else
		{
			// Remove reps with closed channels
			iterator = representative_register.probable_reps.get<nano::representative_register::tag_last_request> ().erase (iterator);
		}
	}
}

std::vector<nano::representative> nano::rep_crawler::representatives (std::size_t count_a, nano::uint128_t const weight_a, boost::optional<decltype (nano::network_constants::protocol_version)> const & opt_version_min_a)
{
	auto version_min (opt_version_min_a.value_or (node.network_params.network.protocol_version_min));
	std::multimap<nano::amount, representative, std::greater<nano::amount>> ordered;
	nano::lock_guard<nano::mutex> lock{ representative_register.probable_reps_mutex };
	for (auto i (representative_register.probable_reps.get<nano::representative_register::tag_account> ().begin ()), n (representative_register.probable_reps.get<nano::representative_register::tag_account> ().end ()); i != n; ++i)
	{
		auto weight = node.ledger.weight (i->get_account ());
		if (weight > weight_a && i->get_channel ()->get_network_version () >= version_min)
		{
			ordered.insert ({ nano::amount{ weight }, *i });
		}
	}
	std::vector<nano::representative> result;
	for (auto i = ordered.begin (), n = ordered.end (); i != n && result.size () < count_a; ++i)
	{
		result.push_back (i->second);
	}
	return result;
}

std::vector<nano::representative> nano::rep_crawler::principal_representatives (std::size_t count_a, boost::optional<decltype (nano::network_constants::protocol_version)> const & opt_version_min_a)
{
	return representatives (count_a, node.minimum_principal_weight (), opt_version_min_a);
}

std::vector<std::shared_ptr<nano::transport::channel>> nano::rep_crawler::representative_endpoints (std::size_t count_a)
{
	std::vector<std::shared_ptr<nano::transport::channel>> result;
	auto reps (representatives (count_a));
	for (auto const & rep : reps)
	{
		result.push_back (rep.get_channel ());
	}
	return result;
}

/** Total number of representatives */
std::size_t nano::rep_crawler::representative_count ()
{
	nano::lock_guard<nano::mutex> lock{ representative_register.probable_reps_mutex };
	return representative_register.probable_reps.size ();
}
