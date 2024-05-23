#include "nano/lib/rsnano.hpp"

#include <nano/lib/blocks.hpp>
#include <nano/lib/stats.hpp>
#include <nano/lib/tomlconfig.hpp>
#include <nano/node/active_transactions.hpp>
#include <nano/node/election_behavior.hpp>
#include <nano/node/node.hpp>
#include <nano/node/scheduler/optimistic.hpp>
#include <nano/secure/ledger.hpp>

nano::scheduler::optimistic::optimistic (optimistic_config const & config_a, nano::node & node_a, nano::ledger & ledger_a, nano::active_transactions & active_a, nano::network_constants const & network_constants_a, nano::stats & stats_a)
{
	auto config_dto{ config_a.into_dto () };
	auto constants_dto{ network_constants_a.to_dto () };
	handle = rsnano::rsn_optimistic_scheduler_create (&config_dto, stats_a.handle, active_a.handle, &constants_dto, ledger_a.handle, node_a.confirming_set.handle);
}

nano::scheduler::optimistic::optimistic (rsnano::OptimisticSchedulerHandle * handle) :
	handle{ handle }
{
}

nano::scheduler::optimistic::~optimistic ()
{
	rsnano::rsn_optimistic_scheduler_destroy (handle);
}

void nano::scheduler::optimistic::start ()
{
	rsnano::rsn_optimistic_scheduler_start (handle);
}

void nano::scheduler::optimistic::stop ()
{
	rsnano::rsn_optimistic_scheduler_stop (handle);
}

void nano::scheduler::optimistic::notify ()
{
	rsnano::rsn_optimistic_scheduler_notify (handle);
}

bool nano::scheduler::optimistic::activate (const nano::account & account, const nano::account_info & account_info, const nano::confirmation_height_info & conf_info)
{
	return rsnano::rsn_optimistic_scheduler_activate (handle, account.bytes.data (), account_info.handle, &conf_info.dto);
}

/*
 * optimistic_scheduler_config
 */

nano::scheduler::optimistic_config::optimistic_config ()
{
	rsnano::OptimisticSchedulerConfigDto dto;
	rsnano::rsn_optimistic_scheduler_config_create (&dto);
	load_dto (dto);
}

void nano::scheduler::optimistic_config::load_dto (rsnano::OptimisticSchedulerConfigDto const & dto_a)
{
	enabled = dto_a.enabled;
	gap_threshold = dto_a.gap_threshold;
	max_size = dto_a.max_size;
}

rsnano::OptimisticSchedulerConfigDto nano::scheduler::optimistic_config::into_dto () const
{
	rsnano::OptimisticSchedulerConfigDto dto;
	dto.enabled = enabled;
	dto.gap_threshold = gap_threshold;
	dto.max_size = max_size;
	return dto;
}

nano::error nano::scheduler::optimistic_config::deserialize (nano::tomlconfig & toml)
{
	toml.get ("enabled", enabled);
	toml.get ("gap_threshold", gap_threshold);
	toml.get ("max_size", max_size);

	return toml.get_error ();
}
