#include <nano/lib/blocks.hpp>
#include <nano/lib/rsnano.hpp>
#include <nano/node/node.hpp>
#include <nano/node/active_transactions.hpp>
#include <nano/node/election.hpp>
#include <nano/node/scheduler/buckets.hpp>
#include <nano/node/scheduler/priority.hpp>
#include <nano/secure/ledger.hpp>

nano::scheduler::priority::priority (nano::node & node_a, nano::stats & stats_a) :
	handle{ rsnano::rsn_election_scheduler_create (this) },
	node{ node_a },
	stats{ stats_a },
	buckets{ std::make_unique<scheduler::buckets> () }
{
}

nano::scheduler::priority::~priority ()
{
	// Thread must be stopped before destruction
	debug_assert (!thread.joinable ());
	rsnano::rsn_election_scheduler_destroy (handle);
}

void nano::scheduler::priority::start ()
{
	debug_assert (!thread.joinable ());

	thread = std::thread{ [this] () {
		nano::thread_role::set (nano::thread_role::name::scheduler_priority);
		run ();
	} };
}

void nano::scheduler::priority::stop ()
{
	{
		nano::lock_guard<nano::mutex> lock{ mutex };
		stopped = true;
	}
	notify ();
	nano::join_or_pass (thread);
}

bool nano::scheduler::priority::activate (nano::account const & account_a, store::transaction const & transaction)
{
	debug_assert (!account_a.is_zero ());
	auto info = node.ledger.account_info (transaction, account_a);
	if (info)
	{
		nano::confirmation_height_info conf_info;
		node.store.confirmation_height ().get (transaction, account_a, conf_info);
		if (conf_info.height () < info->block_count ())
		{
			debug_assert (conf_info.frontier () != info->head ());
			auto hash = conf_info.height () == 0 ? info->open_block () : node.ledger.successor (transaction, conf_info.frontier ()).value_or (0);
			auto block = node.ledger.block (transaction, hash);
			debug_assert (block != nullptr);
			if (node.ledger.dependents_confirmed (transaction, *block))
			{
				auto const balance = node.ledger.balance (transaction, hash).value ();
				auto const previous_balance = node.ledger.balance (transaction, conf_info.frontier ()).value_or (0);
				auto const balance_priority = std::max (balance, previous_balance);

				stats.inc (nano::stat::type::election_scheduler, nano::stat::detail::activated);
				node.logger->trace (nano::log::type::election_scheduler, nano::log::detail::block_activated,
				nano::log::arg{ "account", account_a.to_account () }, // TODO: Convert to lazy eval
				nano::log::arg{ "block", block },
				nano::log::arg{ "time", info->modified () },
				nano::log::arg{ "priority", balance_priority });

				nano::lock_guard<nano::mutex> lock{ mutex };
				buckets->push (info->modified (), block, balance_priority);
				notify ();

				return true; // Activated
			}
		}
	}
	return false; // Not activated
}

void nano::scheduler::priority::notify ()
{
	condition.notify_all ();
}

std::size_t nano::scheduler::priority::size () const
{
	nano::lock_guard<nano::mutex> lock{ mutex };
	return buckets->size ();
}

bool nano::scheduler::priority::empty_locked () const
{
	return buckets->empty ();
}

bool nano::scheduler::priority::empty () const
{
	nano::lock_guard<nano::mutex> lock{ mutex };
	return empty_locked ();
}

bool nano::scheduler::priority::predicate () const
{
	return node.active.vacancy () > 0 && !buckets->empty ();
}

void nano::scheduler::priority::run ()
{
	std::weak_ptr<nano::node> node_w{ node.shared () };
	node.active.on_block_confirmed ([this] (std::shared_ptr<nano::block> const & block, nano::store::read_transaction const & txn, nano::election_status_type status) {
		try_schedule_successors (block, txn, status);
	});

	nano::unique_lock<nano::mutex> lock{ mutex };
	while (!stopped)
	{
		condition.wait (lock, [this] () {
			return stopped || predicate ();
		});
		debug_assert ((std::this_thread::yield (), true)); // Introduce some random delay in debug builds
		if (!stopped)
		{
			stats.inc (nano::stat::type::election_scheduler, nano::stat::detail::loop);

			if (predicate ())
			{
				auto block = buckets->top ();
				buckets->pop ();
				lock.unlock ();
				stats.inc (nano::stat::type::election_scheduler, nano::stat::detail::insert_priority);
				auto result = node.active.insert (block);
				if (result.inserted)
				{
					stats.inc (nano::stat::type::election_scheduler, nano::stat::detail::insert_priority_success);
				}
				if (result.election != nullptr)
				{
					result.election->transition_active ();
				}
			}
			else
			{
				lock.unlock ();
			}
			notify ();
			lock.lock ();
		}
	}
}

void nano::scheduler::priority::try_schedule_successors (std::shared_ptr<nano::block> const & block, nano::store::read_transaction const & transaction, nano::election_status_type status)
{
	auto account = block->account ();
	bool cemented_bootstrap_count_reached = node.ledger.cache.cemented_count () >= node.ledger.get_bootstrap_weight_max_blocks ();
	bool was_active = status == nano::election_status_type::active_confirmed_quorum || status == nano::election_status_type::active_confirmation_height;

	// Next-block activations are only done for blocks with previously active elections
	if (cemented_bootstrap_count_reached && was_active)
	{
		activate_successors (transaction, block);
	}
}

void nano::scheduler::priority::activate_successors (nano::store::read_transaction const & transaction, std::shared_ptr<nano::block> const & block)
{
	activate (block->account (), transaction);

	// Start or vote for the next unconfirmed block in the destination account
	if (block->is_send () && !block->destination ().is_zero () && block->destination () != block->account ())
	{
		activate (block->destination (), transaction);
	}
}

std::unique_ptr<nano::container_info_component> nano::scheduler::priority::collect_container_info (std::string const & name)
{
	nano::unique_lock<nano::mutex> lock{ mutex };

	auto composite = std::make_unique<container_info_composite> (name);
	composite->add_component (buckets->collect_container_info ("buckets"));
	return composite;
}
