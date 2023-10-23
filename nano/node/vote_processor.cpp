#include "nano/lib/rsnano.hpp"
#include "nano/node/transport/tcp.hpp"

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

nano::vote_processor_queue::vote_processor_queue (std::size_t max_votes, nano::stats & stats_a, nano::online_reps & online_reps_a, nano::ledger & ledger_a, std::shared_ptr<nano::logger_mt> & logger_a) :
	handle{ rsnano::rsn_vote_processor_queue_create (max_votes, stats_a.handle, online_reps_a.get_handle (), ledger_a.handle, nano::to_logger_handle (logger_a)) }
{
}

nano::vote_processor_queue::~vote_processor_queue ()
{
	rsnano::rsn_vote_processor_queue_destroy (handle);
}

std::size_t nano::vote_processor_queue::size ()
{
	return rsnano::rsn_vote_processor_queue_len (handle);
}

bool nano::vote_processor_queue::empty ()
{
	return rsnano::rsn_vote_processor_queue_is_empty (handle);
}

bool nano::vote_processor_queue::vote (std::shared_ptr<nano::vote> const & vote_a, std::shared_ptr<nano::transport::channel> const & channel_a)
{
	return rsnano::rsn_vote_processor_queue_vote (handle, vote_a->get_handle (), channel_a->handle);
}

void nano::vote_processor_queue::calculate_weights ()
{
	rsnano::rsn_vote_processor_queue_calculate_weights (handle);
}

bool nano::vote_processor_queue::wait_and_take (std::deque<std::pair<std::shared_ptr<nano::vote>, std::shared_ptr<nano::transport::channel>>> & votes_a)
{
	auto queue_handle = rsnano::rsn_vote_processor_queue_wait_and_take (handle);
	auto len = rsnano::rsn_raw_vote_processor_queue_len (queue_handle);
	votes_a.clear ();
	for (auto i = 0; i < len; ++i)
	{
		rsnano::VoteHandle * vote = nullptr;
		rsnano::ChannelHandle * channel = nullptr;
		rsnano::rsn_raw_vote_processor_queue_get (queue_handle, i, &vote, &channel);
		votes_a.emplace_back (std::make_shared<nano::vote> (vote), nano::transport::channel_handle_to_channel (channel));
	}
	rsnano::rsn_raw_vote_processor_queue_destroy (queue_handle);
	return len > 0;
}

void nano::vote_processor_queue::flush ()
{
	rsnano::rsn_vote_processor_queue_flush (handle);
}

void nano::vote_processor_queue::clear ()
{
	rsnano::rsn_vote_processor_queue_clear (handle);
}

void nano::vote_processor_queue::stop ()
{
	rsnano::rsn_vote_processor_queue_stop (handle);
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (vote_processor_queue & queue, std::string const & name)
{
	auto info_handle = rsnano::rsn_vote_processor_collect_container_info (queue.handle, name.c_str ());
	return std::make_unique<nano::container_info_composite> (info_handle);
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
	while (queue.wait_and_take (votes_l))
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
