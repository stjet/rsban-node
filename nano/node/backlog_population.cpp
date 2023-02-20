#include "nano/lib/rsnano.hpp"
#include "nano/secure/common.hpp"

#include <nano/lib/threading.hpp>
#include <nano/node/backlog_population.hpp>
#include <nano/node/election_scheduler.hpp>
#include <nano/node/nodeconfig.hpp>
#include <nano/secure/store.hpp>

// Helper functions for wrapping the activate callback

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

nano::backlog_population::backlog_population (const config & config_a, nano::ledger & ledger_a, nano::stats & stats_a)
{
	rsnano::BacklogPopulationConfigDto config_dto;
	config_dto.enabled = config_a.enabled;
	config_dto.batch_size = config_a.batch_size;
	config_dto.frequency = config_a.frequency;

	handle = rsnano::rsn_backlog_population_create (&config_dto, ledger_a.get_handle (), stats_a.handle);
}

nano::backlog_population::~backlog_population ()
{
	rsnano::rsn_backlog_population_destroy (handle);
}

void nano::backlog_population::start ()
{
	rsnano::rsn_backlog_population_start (handle);
}

void nano::backlog_population::stop ()
{
	rsnano::rsn_backlog_population_stop (handle);
}

void nano::backlog_population::trigger ()
{
	rsnano::rsn_backlog_population_trigger (handle);
}

void nano::backlog_population::notify ()
{
	rsnano::rsn_backlog_population_notify (handle);
}

void nano::backlog_population::set_activate_callback (std::function<void (nano::transaction const &, nano::account const &, nano::account_info const &, nano::confirmation_height_info const &)> callback_a)
{
	auto callback_ptr = new std::function<void (nano::transaction const &, nano::account const &, nano::account_info const &, nano::confirmation_height_info const &)> (callback_a);
	rsnano::rsn_backlog_population_set_activate_callback (handle, callback_ptr, call_activate_callback, delete_activate_callback);
}
