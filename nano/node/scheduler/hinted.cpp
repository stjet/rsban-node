#include "nano/lib/rsnano.hpp"

#include <nano/lib/stats.hpp>
#include <nano/lib/tomlconfig.hpp>
#include <nano/node/active_transactions.hpp>
#include <nano/node/election_behavior.hpp>
#include <nano/node/node.hpp>
#include <nano/node/scheduler/hinted.hpp>
#include <nano/secure/ledger.hpp>

#include <cstdint>

/*
 * hinted
 */

nano::scheduler::hinted::hinted (nano::scheduler::hinted_config const & config_a, nano::node & node_a, nano::vote_cache & vote_cache_a, nano::active_transactions & active_a, nano::online_reps & online_reps_a, nano::stats & stats_a)
{
	auto config_dto{ config_a.into_dto () };
	handle = rsnano::rsn_hinted_scheduler_create (&config_dto, active_a.handle, node_a.ledger.handle, stats_a.handle, vote_cache_a.handle, node_a.confirming_set.handle, online_reps_a.get_handle ());
}

nano::scheduler::hinted::hinted (rsnano::HintedSchedulerHandle * handle) :
	handle{ handle }
{
}

nano::scheduler::hinted::~hinted ()
{
	rsnano::rsn_hinted_scheduler_destroy (handle);
}

void nano::scheduler::hinted::start ()
{
	rsnano::rsn_hinted_scheduler_start (handle);
}

void nano::scheduler::hinted::stop ()
{
	rsnano::rsn_hinted_scheduler_stop (handle);
}

void nano::scheduler::hinted::notify ()
{
	rsnano::rsn_hinted_scheduler_notify (handle);
}

std::unique_ptr<nano::container_info_component> nano::scheduler::hinted::collect_container_info (const std::string & name) const
{
	return std::make_unique<container_info_composite> (rsnano::rsn_hinted_scheduler_collect_container_info (handle, name.c_str ()));
}

/*
 * hinted_config
 */

nano::scheduler::hinted_config::hinted_config ()
{
	rsnano::HintedSchedulerConfigDto dto;
	rsnano::rsn_hinted_scheduler_config_create (false, &dto);
	load_dto (dto);
}

nano::scheduler::hinted_config::hinted_config (nano::network_constants const & network)
{
	rsnano::HintedSchedulerConfigDto dto;
	rsnano::rsn_hinted_scheduler_config_create (network.is_dev_network (), &dto);
	load_dto (dto);
}

rsnano::HintedSchedulerConfigDto nano::scheduler::hinted_config::into_dto () const
{
	rsnano::HintedSchedulerConfigDto dto;
	dto.enabled = enabled;
	dto.hinting_threshold_percent = hinting_threshold_percent;
	dto.vacancy_threshold_percent = vacancy_threshold_percent;
	dto.check_interval_ms = static_cast<uint32_t> (check_interval.count ());
	dto.block_cooldown_ms = static_cast<uint32_t> (block_cooldown.count ());
	return dto;
}

void nano::scheduler::hinted_config::load_dto (rsnano::HintedSchedulerConfigDto const & dto_a)
{
	enabled = dto_a.enabled;
	check_interval = std::chrono::milliseconds{ dto_a.check_interval_ms };
	block_cooldown = std::chrono::milliseconds{ dto_a.block_cooldown_ms };
	hinting_threshold_percent = dto_a.hinting_threshold_percent;
	vacancy_threshold_percent = dto_a.vacancy_threshold_percent;
}

nano::error nano::scheduler::hinted_config::serialize (nano::tomlconfig & toml) const
{
	toml.put ("enable", enabled, "Enable or disable hinted elections\ntype:bool");
	toml.put ("hinting_threshold", hinting_threshold_percent, "Percentage of online weight needed to start a hinted election. \ntype:uint32,[0,100]");
	toml.put ("check_interval", check_interval.count (), "Interval between scans of the vote cache for possible hinted elections. \ntype:milliseconds");
	toml.put ("block_cooldown", block_cooldown.count (), "Cooldown period for blocks that failed to start an election. \ntype:milliseconds");
	toml.put ("vacancy_threshold", vacancy_threshold_percent, "Percentage of available space in the active elections container needed to trigger a scan for hinted elections (before the check interval elapses). \ntype:uint32,[0,100]");

	return toml.get_error ();
}

nano::error nano::scheduler::hinted_config::deserialize (nano::tomlconfig & toml)
{
	toml.get ("enabled", enabled);
	toml.get ("hinting_threshold", hinting_threshold_percent);

	auto check_interval_l = check_interval.count ();
	toml.get ("check_interval", check_interval_l);
	check_interval = std::chrono::milliseconds{ check_interval_l };

	auto block_cooldown_l = block_cooldown.count ();
	toml.get ("block_cooldown", block_cooldown_l);
	block_cooldown = std::chrono::milliseconds{ block_cooldown_l };

	toml.get ("vacancy_threshold", vacancy_threshold_percent);

	if (hinting_threshold_percent > 100)
	{
		toml.get_error ().set ("hinting_threshold must be a number between 0 and 100");
	}
	if (vacancy_threshold_percent > 100)
	{
		toml.get_error ().set ("vacancy_threshold must be a number between 0 and 100");
	}

	return toml.get_error ();
}
