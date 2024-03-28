#include "nano/lib/rsnano.hpp"
#include "nano/node/transport/tcp.hpp"

#include <nano/lib/logging.hpp>
#include <nano/lib/stats.hpp>
#include <nano/lib/threading.hpp>
#include <nano/lib/timer.hpp>
#include <nano/node/active_transactions.hpp>
#include <nano/node/node_observers.hpp>
#include <nano/node/nodeconfig.hpp>
#include <nano/node/online_reps.hpp>
#include <nano/node/rep_tiers.hpp>
#include <nano/node/repcrawler.hpp>
#include <nano/node/vote_processor.hpp>
#include <nano/secure/common.hpp>
#include <nano/secure/ledger.hpp>

#include <chrono>

using namespace std::chrono_literals;

nano::vote_processor_queue::vote_processor_queue (std::size_t max_votes, nano::stats & stats_a, nano::online_reps & online_reps_a, nano::ledger & ledger_a, nano::rep_tiers & rep_tiers_a)
{
	handle = rsnano::rsn_vote_processor_queue_create (max_votes, stats_a.handle, online_reps_a.get_handle (), ledger_a.handle, rep_tiers_a.handle);
}

nano::vote_processor_queue::~vote_processor_queue ()
{
	rsnano::rsn_vote_processor_queue_destroy (handle);
}

std::size_t nano::vote_processor_queue::size () const
{
	return rsnano::rsn_vote_processor_queue_len (handle);
}

bool nano::vote_processor_queue::empty () const
{
	return rsnano::rsn_vote_processor_queue_is_empty (handle);
}

bool nano::vote_processor_queue::vote (std::shared_ptr<nano::vote> const & vote_a, std::shared_ptr<nano::transport::channel> const & channel_a)
{
	return rsnano::rsn_vote_processor_queue_vote (handle, vote_a->get_handle (), channel_a->handle);
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
nano::active_transactions & active_a,
nano::node_observers & observers_a,
nano::stats & stats_a,
nano::node_config & config_a,
nano::logger & logger_a,
nano::rep_crawler & rep_crawler_a,
nano::network_params & network_params_a,
nano::rep_tiers & rep_tiers_a) :
	active{ active_a },
	observers{ observers_a },
	stats{ stats_a },
	config{ config_a },
	logger{ logger_a },
	rep_crawler{ rep_crawler_a },
	network_params{ network_params_a },
	rep_tiers{ rep_tiers_a },
	queue{ queue_a }
{
}

nano::vote_processor::~vote_processor ()
{
	// Thread must be stopped before destruction
	debug_assert (!thread.joinable ());
}

void nano::vote_processor::start ()
{
	debug_assert (!thread.joinable ());

	thread = std::thread{ [this] () {
		nano::thread_role::set (nano::thread_role::name::vote_processing);
		run ();
	} };
}

void nano::vote_processor::stop ()
{
	queue.stop ();
	{
		nano::lock_guard<nano::mutex> lock{ mutex };
		stopped = true;
	}
	if (thread.joinable ())
	{
		thread.join ();
	}
}

void nano::vote_processor::run ()
{
	nano::timer<std::chrono::milliseconds> elapsed;
	bool log_this_iteration;

	nano::unique_lock<nano::mutex> lock{ mutex };
	std::deque<std::pair<std::shared_ptr<nano::vote>, std::shared_ptr<nano::transport::channel>>> votes_l;
	while (queue.wait_and_take (votes_l))
	{
		log_this_iteration = false;
		// TODO: This is a temporary measure to prevent spamming the logs until we can implement a better solution
		if (votes_l.size () > 1024 * 4)
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
			logger.debug (nano::log::type::vote_processor, "Processed {} votes in {} milliseconds (rate of {} votes per second)",
			votes_l.size (),
			elapsed.value ().count (),
			((votes_l.size () * 1000ULL) / elapsed.value ().count ()));
		}
	}
}

void nano::vote_processor::verify_votes (std::deque<std::pair<std::shared_ptr<nano::vote>, std::shared_ptr<nano::transport::channel>>> const & votes_a)
{
	for (auto const & vote : votes_a)
	{
		if (!nano::validate_message (vote.first->account (), vote.first->hash (), vote.first->signature ()))
		{
			vote_blocking (vote.first, vote.second, true);
		}
	}
}

nano::vote_code nano::vote_processor::vote_blocking (std::shared_ptr<nano::vote> const & vote, std::shared_ptr<nano::transport::channel> const & channel_a, bool validated)
{
	auto result = nano::vote_code::invalid;
	if (validated || !vote->validate ())
	{
		auto vote_results = active.vote (vote);

		// Aggregate results for individual hashes
		bool replay = false;
		bool processed = false;
		for (auto const & [hash, hash_result] : vote_results)
		{
			replay |= (hash_result == nano::vote_code::replay);
			processed |= (hash_result == nano::vote_code::vote);
		}
		result = replay ? nano::vote_code::replay : (processed ? nano::vote_code::vote : nano::vote_code::indeterminate);

		observers.vote.notify (vote, channel_a, result);
	}

	stats.inc (nano::stat::type::vote, to_stat_detail (result));

	logger.trace (nano::log::type::vote_processor, nano::log::detail::vote_processed,
	nano::log::arg{ "vote", vote },
	nano::log::arg{ "result", result });

	return result;
}
