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

#include <memory>

using namespace std::chrono_literals;

nano::vote_processor_queue::vote_processor_queue (std::size_t max_votes, nano::stats & stats_a, nano::online_reps & online_reps_a, nano::ledger & ledger_a, nano::rep_tiers & rep_tiers_a)
{
	handle = rsnano::rsn_vote_processor_queue_create (max_votes, stats_a.handle, online_reps_a.get_handle (), ledger_a.handle, rep_tiers_a.handle);
}

nano::vote_processor_queue::vote_processor_queue (rsnano::VoteProcessorQueueHandle * handle) :
	handle{ handle }
{
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

namespace
{
void on_vote_processed (void * context, rsnano::VoteHandle * vote_handle, rsnano::ChannelHandle * channel_handle, uint8_t code)
{
	auto observers = static_cast<std::shared_ptr<nano::node_observers> *> (context);
	auto vote = std::make_shared<nano::vote> (vote_handle);
	auto channel = nano::transport::channel_handle_to_channel (channel_handle);
	(*observers)->vote.notify (vote, channel, static_cast<nano::vote_code> (code));
}

void delete_vote_processed (void * context)
{
	auto observers = static_cast<std::shared_ptr<nano::node_observers> *> (context);
	delete observers;
}
}

nano::vote_processor::vote_processor (
nano::vote_processor_queue & queue_a,
nano::active_transactions & active_a,
std::shared_ptr<nano::node_observers> observers_a,
nano::stats & stats_a,
nano::node_config & config_a,
nano::logger & logger_a,
nano::rep_crawler & rep_crawler_a,
nano::network_params & network_params_a,
nano::rep_tiers & rep_tiers_a)
{
	auto context = new std::shared_ptr<nano::node_observers> (observers_a);
	handle = rsnano::rsn_vote_processor_create (queue_a.handle, active_a.handle, stats_a.handle, on_vote_processed, context, delete_vote_processed);
}

nano::vote_processor::~vote_processor ()
{
	rsnano::rsn_vote_processor_destroy (handle);
}

void nano::vote_processor::start ()
{
	rsnano::rsn_vote_processor_start (handle);
}

void nano::vote_processor::stop ()
{
	rsnano::rsn_vote_processor_stop (handle);
}

uint64_t nano::vote_processor::total_processed () const
{
	return rsnano::rsn_vote_processor_total_processed (handle);
}

nano::vote_code nano::vote_processor::vote_blocking (std::shared_ptr<nano::vote> const & vote, std::shared_ptr<nano::transport::channel> const & channel_a, bool validated)
{
	return static_cast<nano::vote_code> (rsnano::rsn_vote_processor_vote_blocking (handle, vote->get_handle (), channel_a->handle, validated));
}
