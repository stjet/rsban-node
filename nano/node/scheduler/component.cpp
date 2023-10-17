#include <nano/node/node.hpp>
#include <nano/node/scheduler/component.hpp>
#include <nano/node/scheduler/hinted.hpp>
#include <nano/node/scheduler/manual.hpp>
#include <nano/node/scheduler/optimistic.hpp>
#include <nano/node/scheduler/priority.hpp>

nano::scheduler::component::component (nano::node & node) :
	hinted_impl{ std::make_unique<nano::scheduler::hinted> (nano::scheduler::hinted::config{ *node.config }, node, node.inactive_vote_cache, node.active, node.online_reps, *node.stats) },
	manual_impl{ std::make_unique<nano::scheduler::manual> (node) },
	optimistic_impl{ std::make_unique<nano::scheduler::optimistic> (node.config->optimistic_scheduler, node, node.ledger, node.active, node.network_params.network, *node.stats) },
	priority_impl{ std::make_unique<nano::scheduler::priority> (node, *node.stats) },
	hinted{ *hinted_impl },
	manual{ *manual_impl },
	optimistic{ *optimistic_impl },
	priority{ *priority_impl }
{
}

nano::scheduler::component::~component ()
{
}

void nano::scheduler::component::start ()
{
	hinted.start ();
	manual.start ();
	optimistic.start ();
	priority.start ();
}

void nano::scheduler::component::stop ()
{
	hinted.stop ();
	manual.stop ();
	optimistic.stop ();
	priority.stop ();
}

std::unique_ptr<nano::container_info_component> nano::scheduler::component::collect_container_info (std::string const & name)
{
	nano::unique_lock<nano::mutex> lock{ mutex };

	auto composite = std::make_unique<container_info_composite> (name);
	//composite->add_component (hinted.collect_container_info ("hinted"));
	composite->add_component (manual.collect_container_info ("manual"));
	//composite->add_component (optimistic.collect_container_info ("optimistic"));
	composite->add_component (priority.collect_container_info ("priority"));
	return composite;
}

nano::scheduler::successor_scheduler::successor_scheduler (nano::node & node) :
	node{ node }
{
}

void nano::scheduler::successor_scheduler::schedule (std::shared_ptr<nano::block> const & block, nano::store::read_transaction const & transaction, nano::election_status_type status)
{
	auto const account = !block->account ().is_zero () ? block->account () : block->sideband ().account ();
	bool cemented_bootstrap_count_reached = node.ledger.cache.cemented_count () >= node.ledger.get_bootstrap_weight_max_blocks ();
	bool was_active = status == nano::election_status_type::active_confirmed_quorum || status == nano::election_status_type::active_confirmation_height;

	// Next-block activations are only done for blocks with previously active elections
	if (cemented_bootstrap_count_reached && was_active)
	{
		activate_successors (account, block, transaction);
	}
}

void nano::scheduler::successor_scheduler::activate_successors (const nano::account & account, std::shared_ptr<nano::block> const & block, nano::store::read_transaction const & transaction)
{
	node.scheduler.priority.activate (account, transaction);
	auto const destination = node.ledger.block_destination (transaction, *block);

	// Start or vote for the next unconfirmed block in the destination account
	if (!destination.is_zero () && destination != account)
	{
		node.scheduler.priority.activate (destination, transaction);
	}
}
