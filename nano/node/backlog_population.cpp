#include <nano/lib/rsnano.hpp>
#include <nano/lib/threading.hpp>
#include <nano/node/backlog_population.hpp>
#include <nano/node/nodeconfig.hpp>
#include <nano/node/scheduler/priority.hpp>
#include <nano/secure/account_info.hpp>
#include <nano/secure/common.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/store/component.hpp>
#include <nano/store/transaction.hpp>

// Helper functions for wrapping the activate callback

namespace
{
void call_activate_callback (void * context, rsnano::TransactionHandle * txn_handle, const uint8_t * account_ptr)
{
	auto callback = static_cast<std::function<void (nano::store::transaction const &, nano::account const &)> *> (context);

	nano::account account;
	std::copy (account_ptr, account_ptr + 32, std::begin (account.bytes));

	(*callback) (nano::store::transaction_wrapper{ txn_handle }, account);
}

void delete_activate_callback (void * callback_ptr)
{
	auto callback = static_cast<std::function<void (nano::store::transaction const &, nano::account const &)> *> (callback_ptr);
	delete callback;
}
}

nano::backlog_population::backlog_population (rsnano::BacklogPopulationHandle * handle) :
	handle{ handle }
{
}

nano::backlog_population::~backlog_population ()
{
	rsnano::rsn_backlog_population_destroy (handle);
}

void nano::backlog_population::trigger ()
{
	rsnano::rsn_backlog_population_trigger (handle);
}

void nano::backlog_population::set_activate_callback (std::function<void (nano::store::transaction const &, nano::account const &)> callback_a)
{
	auto callback_ptr = new std::function<void (nano::store::transaction const &, nano::account const &)> (callback_a);
	rsnano::rsn_backlog_population_set_activate_callback (handle, callback_ptr, call_activate_callback, delete_activate_callback);
}
