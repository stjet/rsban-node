#include <nano/lib/logger_mt.hpp>
#include <nano/lib/stats.hpp>
#include <nano/lib/threading.hpp>
#include <nano/lib/timer.hpp>
#include <nano/node/active_transactions.hpp>
#include <nano/node/node_observers.hpp>
#include <nano/node/nodeconfig.hpp>
#include <nano/node/online_reps.hpp>
#include <nano/node/repcrawler.hpp>
#include <nano/node/signatures.hpp>
#include <nano/node/vote_processor.hpp>
#include <nano/secure/common.hpp>
#include <nano/secure/ledger.hpp>

#include <boost/format.hpp>

#include <chrono>
using namespace std::chrono_literals;

nano::vote_processor_queue::vote_processor_queue (std::size_t max_votes, nano::stats & stats_a, nano::online_reps & online_reps_a, nano::ledger & ledger_a, nano::logger_mt & logger_a) :
	max_votes{ max_votes },
	stats (stats_a),
	online_reps{ online_reps_a },
	ledger{ ledger_a },
	logger{ logger_a },
	stopped (false)
{
}

std::size_t nano::vote_processor_queue::size ()
{
	nano::lock_guard<nano::mutex> guard{ mutex };
	return votes.size ();
}

bool nano::vote_processor_queue::empty ()
{
	nano::lock_guard<nano::mutex> guard{ mutex };
	return votes.empty ();
}

bool nano::vote_processor_queue::half_full ()
{
	return size () >= max_votes / 2;
}

bool nano::vote_processor_queue::vote (std::shared_ptr<nano::vote> const & vote_a, std::shared_ptr<nano::transport::channel> const & channel_a)
{
	debug_assert (channel_a != nullptr);
	bool process (false);
	nano::unique_lock<nano::mutex> lock{ mutex };
	if (!stopped)
	{
		// Level 0 (< 0.1%)
		if (votes.size () < 6.0 / 9.0 * max_votes)
		{
			process = true;
		}
		// Level 1 (0.1-1%)
		else if (votes.size () < 7.0 / 9.0 * max_votes)
		{
			process = (representatives_1.find (vote_a->account ()) != representatives_1.end ());
		}
		// Level 2 (1-5%)
		else if (votes.size () < 8.0 / 9.0 * max_votes)
		{
			process = (representatives_2.find (vote_a->account ()) != representatives_2.end ());
		}
		// Level 3 (> 5%)
		else if (votes.size () < max_votes)
		{
			process = (representatives_3.find (vote_a->account ()) != representatives_3.end ());
		}
		if (process)
		{
			votes.emplace_back (vote_a, channel_a);
			lock.unlock ();
			condition.notify_all ();
			// Lock no longer required
		}
		else
		{
			stats.inc (nano::stat::type::vote, nano::stat::detail::vote_overflow);
		}
	}
	return !process;
}

void nano::vote_processor_queue::calculate_weights ()
{
	nano::unique_lock<nano::mutex> lock{ mutex };
	if (!stopped)
	{
		representatives_1.clear ();
		representatives_2.clear ();
		representatives_3.clear ();
		auto supply (online_reps.trended ());
		auto rep_amounts = ledger.cache.rep_weights ().get_rep_amounts ();
		for (auto const & rep_amount : rep_amounts)
		{
			nano::account const & representative (rep_amount.first);
			auto weight (ledger.weight (representative));
			if (weight > supply / 1000) // 0.1% or above (level 1)
			{
				representatives_1.insert (representative);
				if (weight > supply / 100) // 1% or above (level 2)
				{
					representatives_2.insert (representative);
					if (weight > supply / 20) // 5% or above (level 3)
					{
						representatives_3.insert (representative);
					}
				}
			}
		}
	}
}

bool nano::vote_processor_queue::wait_and_swap (std::deque<std::pair<std::shared_ptr<nano::vote>, std::shared_ptr<nano::transport::channel>>> & votes_a)
{
	nano::unique_lock<nano::mutex> lock{ mutex };
	while (true)
	{
		if (stopped)
		{
			return false;
		}

		if (!votes.empty ())
		{
			votes_a.swap (votes);
			lock.unlock ();
			condition.notify_all ();
			return true;
		}
		else
		{
			condition.wait (lock);
		}
	}
}

void nano::vote_processor_queue::flush ()
{
	nano::unique_lock<nano::mutex> lock{ mutex };
	bool success = condition.wait_for (lock, 60s, [this] () {
		return stopped || votes.empty () /*|| total_processed.load (std::memory_order_relaxed) >= cutoff*/;
	});
	if (!success)
	{
		logger.always_log ("WARNING: vote_processor_queue::flush timeout while waiting for flush");
		debug_assert (false && "vote_processor_queue::flush timeout while waiting for flush");
	}
}

void nano::vote_processor_queue::clear ()
{
	nano::unique_lock<nano::mutex> lock{ mutex };
	votes.clear ();
	condition.notify_all ();
}

void nano::vote_processor_queue::stop ()
{
	{
		nano::lock_guard<nano::mutex> lock{ mutex };
		stopped = true;
	}
	condition.notify_all ();
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (vote_processor_queue & queue, std::string const & name)
{
	std::size_t votes_count;
	std::size_t representatives_1_count;
	std::size_t representatives_2_count;
	std::size_t representatives_3_count;

	{
		nano::lock_guard<nano::mutex> guard{ queue.mutex };
		votes_count = queue.votes.size ();
		representatives_1_count = queue.representatives_1.size ();
		representatives_2_count = queue.representatives_2.size ();
		representatives_3_count = queue.representatives_3.size ();
	}

	auto composite = std::make_unique<container_info_composite> (name);
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "votes", votes_count, sizeof (decltype (queue.votes)::value_type) }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "representatives_1", representatives_1_count, sizeof (decltype (queue.representatives_1)::value_type) }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "representatives_2", representatives_2_count, sizeof (decltype (queue.representatives_2)::value_type) }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "representatives_3", representatives_3_count, sizeof (decltype (queue.representatives_3)::value_type) }));
	return composite;
}

nano::vote_processor::vote_processor (
nano::vote_processor_queue & queue_a,
nano::signature_checker & checker_a,
nano::active_transactions & active_a,
nano::node_observers & observers_a,
nano::stats & stats_a,
nano::node_config & config_a,
nano::logger_mt & logger_a,
nano::rep_crawler & rep_crawler_a,
nano::network_params & network_params_a) :
	checker (checker_a),
	active (active_a),
	observers (observers_a),
	stats (stats_a),
	config (config_a),
	logger (logger_a),
	rep_crawler (rep_crawler_a),
	network_params (network_params_a),
	started (false),
	queue{ queue_a },
	thread ([this] () {
		nano::thread_role::set (nano::thread_role::name::vote_processing);
		process_loop ();
		queue.clear ();
	})
{
	nano::unique_lock<nano::mutex> lock{ mutex };
	condition.wait (lock, [&started = started] { return started; });
}

void nano::vote_processor::process_loop ()
{
	nano::timer<std::chrono::milliseconds> elapsed;
	bool log_this_iteration;

	nano::unique_lock<nano::mutex> lock{ mutex };
	started = true;
	lock.unlock ();
	condition.notify_all ();

	std::deque<std::pair<std::shared_ptr<nano::vote>, std::shared_ptr<nano::transport::channel>>> votes_l;
	while (queue.wait_and_swap (votes_l))
	{
		log_this_iteration = false;
		if (config.logging.network_logging () && votes_l.size () > 50)
		{
			/*
				* Only log the timing information for this iteration if
				* there are a sufficient number of items for it to be relevant
				*/
			log_this_iteration = true;
			elapsed.restart ();
		}
		verify_votes (votes_l);
		total_processed += votes_l.size ();
		votes_l.clear ();

		if (log_this_iteration && elapsed.stop () > std::chrono::milliseconds (100))
		{
			logger.try_log (boost::str (boost::format ("Processed %1% votes in %2% milliseconds (rate of %3% votes per second)") % votes_l.size () % elapsed.value ().count () % ((votes_l.size () * 1000ULL) / elapsed.value ().count ())));
		}
	}
}

// TODO call queue.vote() directly
bool nano::vote_processor::vote (std::shared_ptr<nano::vote> const & vote_a, std::shared_ptr<nano::transport::channel> const & channel_a)
{
	return queue.vote (vote_a, channel_a);
}

void nano::vote_processor::verify_votes (std::deque<std::pair<std::shared_ptr<nano::vote>, std::shared_ptr<nano::transport::channel>>> const & votes_a)
{
	auto size (votes_a.size ());
	std::vector<unsigned char const *> messages;
	messages.reserve (size);
	std::vector<nano::block_hash> hashes;
	hashes.reserve (size);
	std::vector<std::size_t> lengths (size, sizeof (nano::block_hash));
	std::vector<unsigned char const *> pub_keys;
	pub_keys.reserve (size);
	std::vector<unsigned char const *> signatures;
	signatures.reserve (size);
	std::vector<int> verifications;
	verifications.resize (size);
	std::vector<nano::account> tmp_accounts;
	tmp_accounts.reserve (size);
	std::vector<nano::signature> tmp_signatures;
	tmp_signatures.reserve (size);
	for (auto const & vote : votes_a)
	{
		hashes.push_back (vote.first->hash ());
		messages.push_back (hashes.back ().bytes.data ());
		tmp_accounts.push_back (vote.first->account ());
		tmp_signatures.push_back (vote.first->signature ());
		pub_keys.push_back (tmp_accounts.back ().bytes.data ());
		signatures.push_back (tmp_signatures.back ().bytes.data ());
	}
	nano::signature_check_set check = { size, messages.data (), lengths.data (), pub_keys.data (), signatures.data (), verifications.data () };
	checker.verify (check);
	auto i (0);
	for (auto const & vote : votes_a)
	{
		debug_assert (verifications[i] == 1 || verifications[i] == 0);
		if (verifications[i] == 1)
		{
			vote_blocking (vote.first, vote.second, true);
		}
		++i;
	}
}

nano::vote_code nano::vote_processor::vote_blocking (std::shared_ptr<nano::vote> const & vote_a, std::shared_ptr<nano::transport::channel> const & channel_a, bool validated)
{
	auto result (nano::vote_code::invalid);
	if (validated || !vote_a->validate ())
	{
		result = active.vote (vote_a);
		observers.vote.notify (vote_a, channel_a, result);
	}
	std::string status;
	switch (result)
	{
		case nano::vote_code::invalid:
			status = "Invalid";
			stats.inc (nano::stat::type::vote, nano::stat::detail::vote_invalid);
			break;
		case nano::vote_code::replay:
			status = "Replay";
			stats.inc (nano::stat::type::vote, nano::stat::detail::vote_replay);
			break;
		case nano::vote_code::vote:
			status = "Vote";
			stats.inc (nano::stat::type::vote, nano::stat::detail::vote_valid);
			break;
		case nano::vote_code::indeterminate:
			status = "Indeterminate";
			stats.inc (nano::stat::type::vote, nano::stat::detail::vote_indeterminate);
			break;
	}
	if (config.logging.vote_logging ())
	{
		logger.try_log (boost::str (boost::format ("Vote from: %1% timestamp: %2% duration %3%ms block(s): %4% status: %5%") % vote_a->account ().to_account () % std::to_string (vote_a->timestamp ()) % std::to_string (vote_a->duration ().count ()) % vote_a->hashes_string () % status));
	}
	return result;
}

void nano::vote_processor::stop ()
{
	queue.stop ();
	if (thread.joinable ())
	{
		thread.join ();
	}
}

void nano::vote_processor::flush ()
{
	queue.flush ();
}

std::size_t nano::vote_processor::size ()
{
	return queue.size ();
}

bool nano::vote_processor::empty ()
{
	return queue.empty ();
}

bool nano::vote_processor::half_full ()
{
	return queue.half_full ();
}

void nano::vote_processor::calculate_weights ()
{
	queue.calculate_weights ();
}
