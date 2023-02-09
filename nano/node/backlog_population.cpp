#include "nano/lib/rsnano.hpp"
#include "nano/secure/common.hpp"

#include <nano/lib/threading.hpp>
#include <nano/node/backlog_population.hpp>
#include <nano/node/election_scheduler.hpp>
#include <nano/node/nodeconfig.hpp>
#include <nano/secure/store.hpp>

nano::backlog_population::backlog_population (const config & config_a, nano::store & store_a, nano::stats & stats_a) :
	config_m{ config_a },
	store{ store_a },
	stats{ stats_a },
	handle{ rsnano::rsn_backlog_population_create (store.get_handle (), stats.handle) }
{
}

nano::backlog_population::~backlog_population ()
{
	// Thread must be stopped before destruction
	debug_assert (!thread.joinable ());
	rsnano::rsn_backlog_population_destroy (handle);
}

void nano::backlog_population::start ()
{
	debug_assert (!thread.joinable ());

	thread = std::thread{ [this] () {
		nano::thread_role::set (nano::thread_role::name::backlog_population);
		run ();
	} };
}

void nano::backlog_population::stop ()
{
	nano::unique_lock<nano::mutex> lock{ mutex };
	stopped = true;
	lock.unlock ();
	notify ();
	nano::join_or_pass (thread);
}

void nano::backlog_population::trigger ()
{
	{
		nano::unique_lock<nano::mutex> lock{ mutex };
		triggered = true;
	}
	notify ();
}

void nano::backlog_population::notify ()
{
	condition.notify_all ();
}

namespace
{
void call_activate_callback (void * context, rsnano::TransactionHandle * txn_handle, const uint8_t * account_ptr, rsnano::AccountInfoHandle * account_info_handle, const rsnano::ConfirmationHeightInfoDto * conf_height_dto)
{
	auto callback = static_cast<std::function<void (nano::transaction const &, nano::account const &, nano::account_info const &, nano::confirmation_height_info const &)> *> (context);

	nano::account account;
	std::copy (account_ptr, account_ptr + 32, std::begin (account.bytes));

	nano::account_info account_info{ rsnano::rsn_account_info_clone (account_info_handle) };
	nano::confirmation_height_info conf_height{ *conf_height_dto };

	(*callback) (nano::transaction_wrapper{ txn_handle }, account, account_info, conf_height);
}

void delete_activate_callback (void * callback_ptr)
{
	auto callback = static_cast<std::function<void (nano::transaction const &, nano::account const &, nano::account_info const &, nano::confirmation_height_info const &)> *> (callback_ptr);

	delete callback;
}
}

void nano::backlog_population::set_activate_callback (std::function<void (nano::transaction const &, nano::account const &, nano::account_info const &, nano::confirmation_height_info const &)> callback_a)
{
	activate_callback.add (callback_a);

	auto callback_ptr = new std::function<void (nano::transaction const &, nano::account const &, nano::account_info const &, nano::confirmation_height_info const &)> (callback_a);
	rsnano::rsn_backlog_population_set_activate_callback (handle, callback_ptr, call_activate_callback, delete_activate_callback);
}

bool nano::backlog_population::predicate () const
{
	return triggered || config_m.enabled;
}

void nano::backlog_population::run ()
{
	nano::unique_lock<nano::mutex> lock{ mutex };
	while (!stopped)
	{
		if (predicate ())
		{
			stats.inc (nano::stat::type::backlog, nano::stat::detail::loop);

			triggered = false;
			populate_backlog (lock);
		}

		condition.wait (lock, [this] () {
			return stopped || predicate ();
		});
	}
}

void nano::backlog_population::populate_backlog (nano::unique_lock<nano::mutex> & lock)
{
	debug_assert (config_m.frequency > 0);

	const auto chunk_size = config_m.batch_size / config_m.frequency;
	auto done = false;
	nano::account next = 0;
	uint64_t total = 0;
	while (!stopped && !done)
	{
		lock.unlock ();
		{
			auto transaction = store.tx_begin_read ();

			auto count = 0u;
			auto i = store.account ().begin (*transaction, next);
			auto const end = store.account ().end ();
			for (; i != end && count < chunk_size; ++i, ++count, ++total)
			{
				stats.inc (nano::stat::type::backlog, nano::stat::detail::total);

				auto const & account = i->first;
				activate (*transaction, account);
				next = account.number () + 1;
			}
			done = store.account ().begin (*transaction, next) == end;
		}
		lock.lock ();
		// Give the rest of the node time to progress without holding database lock
		std::this_thread::sleep_for (std::chrono::milliseconds (1000 / config_m.frequency));
	}
}

void nano::backlog_population::activate (nano::transaction const & transaction, nano::account const & account)
{
	rsnano::rsn_backlog_population_activate (handle, transaction.get_rust_handle (), account.bytes.data ());
}
