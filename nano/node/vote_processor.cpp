#include "nano/lib/rsnano.hpp"
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

nano::vote_processor_queue::vote_processor_queue (rsnano::VoteProcessorQueueHandle * handle) :
	handle{ handle }
{
}

nano::vote_processor_queue::~vote_processor_queue ()
{
	rsnano::rsn_vote_processor_queue_destroy (handle);
}

bool nano::vote_processor_queue::empty () const
{
	return rsnano::rsn_vote_processor_queue_is_empty (handle);
}

bool nano::vote_processor_queue::vote (std::shared_ptr<nano::vote> const & vote_a, std::shared_ptr<nano::transport::channel> const & channel_a)
{
	return rsnano::rsn_vote_processor_queue_vote (handle, vote_a->get_handle (), channel_a->handle);
}

void nano::vote_processor_queue::flush ()
{
	rsnano::rsn_vote_processor_queue_flush (handle);
}

nano::vote_processor::vote_processor (rsnano::VoteProcessorHandle * handle) :
	handle{ handle }
{
}

nano::vote_processor::~vote_processor ()
{
	rsnano::rsn_vote_processor_destroy (handle);
}

nano::vote_code nano::vote_processor::vote_blocking (std::shared_ptr<nano::vote> const & vote, std::shared_ptr<nano::transport::channel> const & channel_a, bool validated)
{
	return static_cast<nano::vote_code> (rsnano::rsn_vote_processor_vote_blocking (handle, vote->get_handle (), channel_a->handle, validated));
}
