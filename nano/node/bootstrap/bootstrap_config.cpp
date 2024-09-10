#include "nano/lib/rsnano.hpp"

#include <nano/lib/tomlconfig.hpp>
#include <nano/node/bootstrap/bootstrap_config.hpp>

#include <chrono>

/*
 * account_sets_config
 */
nano::account_sets_config::account_sets_config ()
{
	rsnano::AccountSetsConfigDto dto;
	rsnano::rsn_account_sets_config_create (&dto);
	load_dto (dto);
}

nano::account_sets_config::account_sets_config (rsnano::AccountSetsConfigDto const & dto_a)
{
	load_dto (dto_a);
}

rsnano::AccountSetsConfigDto nano::account_sets_config::to_dto () const
{
	rsnano::AccountSetsConfigDto dto;
	dto.consideration_count = consideration_count;
	dto.priorities_max = priorities_max;
	dto.blocking_max = blocking_max;
	dto.cooldown_ms = cooldown.count ();
	return dto;
}

void nano::account_sets_config::load_dto (rsnano::AccountSetsConfigDto const & dto)
{
	consideration_count = dto.consideration_count;
	priorities_max = dto.priorities_max;
	blocking_max = dto.blocking_max;
	cooldown = std::chrono::milliseconds{ dto.cooldown_ms };
}

nano::error nano::account_sets_config::deserialize (nano::tomlconfig & toml)
{
	toml.get ("consideration_count", consideration_count);
	toml.get ("priorities_max", priorities_max);
	toml.get ("blocking_max", blocking_max);
	toml.get_duration ("cooldown", cooldown);

	return toml.get_error ();
}

/*
 * bootstrap_ascending_config
 */
nano::bootstrap_ascending_config::bootstrap_ascending_config ()
{
	rsnano::BootstrapAscendingConfigDto dto;
	rsnano::rsn_bootstrap_config_create (&dto);
	load_dto (dto);
}

nano::bootstrap_ascending_config::bootstrap_ascending_config (rsnano::BootstrapAscendingConfigDto const & dto_a)
{
	load_dto (dto_a);
}

rsnano::BootstrapAscendingConfigDto nano::bootstrap_ascending_config::to_dto () const
{
	rsnano::BootstrapAscendingConfigDto dto;
	dto.database_rate_limit = database_rate_limit;
	dto.database_warmup_ratio = database_warmup_ratio;
	dto.channel_limit = channel_limit;
	dto.max_pull_count = max_pull_count;
	dto.timeout_ms = request_timeout.count ();
	dto.throttle_coefficient = throttle_coefficient;
	dto.throttle_wait_ms = throttle_wait.count ();
	dto.block_processor_threshold = block_processor_threshold;
	dto.account_sets = account_sets.to_dto ();
	dto.enable = enable;
	dto.enable_database_scan = enable_database_scan;
	dto.enable_dependency_walker = enable_dependency_walker;
	dto.max_requests = max_requests;
	return dto;
}

void nano::bootstrap_ascending_config::load_dto (rsnano::BootstrapAscendingConfigDto const & dto)
{
	database_rate_limit = dto.database_rate_limit;
	database_warmup_ratio = dto.database_warmup_ratio;
	channel_limit = dto.channel_limit;
	max_pull_count = dto.max_pull_count;
	request_timeout = std::chrono::milliseconds{ dto.timeout_ms };
	throttle_coefficient = dto.throttle_coefficient;
	throttle_wait = std::chrono::milliseconds{ dto.throttle_wait_ms };
	block_processor_threshold = dto.block_processor_threshold;
	account_sets.load_dto (dto.account_sets);
	enable = dto.enable;
	enable_database_scan = dto.enable_database_scan;
	enable_dependency_walker = dto.enable_dependency_walker;
	max_requests = dto.max_requests;
}

nano::error nano::bootstrap_ascending_config::deserialize (nano::tomlconfig & toml)
{
	toml.get ("enable", enable);
	toml.get ("enable_database_scan", enable_database_scan);
	toml.get ("enable_dependency_walker", enable_dependency_walker);
	toml.get ("channel_limit", channel_limit);
	toml.get ("database_rate_limit", database_rate_limit);
	toml.get ("database_warmup_ratio", database_warmup_ratio);
	toml.get ("max_pull_count", max_pull_count);
	toml.get_duration ("request_timeout", request_timeout);
	toml.get ("throttle_coefficient", throttle_coefficient);
	toml.get_duration ("throttle_wait", throttle_wait);
	toml.get ("block_processor_threshold", block_processor_threshold);
	toml.get ("max_requests", max_requests);

	if (toml.has_key ("account_sets"))
	{
		auto config_l = toml.get_required_child ("account_sets");
		account_sets.deserialize (config_l);
	}

	return toml.get_error ();
}
