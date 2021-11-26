#include <nano/lib/lmdbconfig.hpp>
#include <nano/lib/tomlconfig.hpp>
#include <nano/secure/common.hpp>

#include <iostream>

nano::lmdb_config::lmdb_config ()
{
	rsnano::LmdbConfigDto dto;
	rsnano::rsn_lmdb_config_create (&dto);
	load_dto (dto);
}

void nano::lmdb_config::load_dto (rsnano::LmdbConfigDto & dto)
{
	switch (dto.sync)
	{
		case 0:
			sync = nano::lmdb_config::sync_strategy::always;
			break;
		case 1:
			sync = nano::lmdb_config::sync_strategy::nosync_safe;
			break;
		case 2:
			sync = nano::lmdb_config::sync_strategy::nosync_unsafe;
			break;
		case 3:
			sync = nano::lmdb_config::sync_strategy::nosync_unsafe_large_memory;
			break;
		default:
			throw std::runtime_error ("unknown sync type");
	}
	max_databases = dto.max_databases;
	map_size = dto.map_size;
}

rsnano::LmdbConfigDto nano::lmdb_config::to_dto () const
{
	rsnano::LmdbConfigDto dto;
	switch (sync)
	{
		case nano::lmdb_config::sync_strategy::always:
			dto.sync = 0;
			break;
		case nano::lmdb_config::sync_strategy::nosync_safe:
			dto.sync = 1;
			break;
		case nano::lmdb_config::sync_strategy::nosync_unsafe:
			dto.sync = 2;
			break;
		case nano::lmdb_config::sync_strategy::nosync_unsafe_large_memory:
			dto.sync = 3;
			break;
		default:
			dto.sync = 0;
			break;
	}
	dto.max_databases = max_databases;
	dto.map_size = map_size;
	return dto;
}

nano::error nano::lmdb_config::deserialize_toml (nano::tomlconfig & toml)
{
	auto default_max_databases = max_databases;
	toml.get_optional<uint32_t> ("max_databases", max_databases);
	toml.get_optional<size_t> ("map_size", map_size);

	if (!toml.get_error ())
	{
		std::string sync_string = "always";
		toml.get_optional<std::string> ("sync", sync_string);
		if (sync_string == "always")
		{
			sync = nano::lmdb_config::sync_strategy::always;
		}
		else if (sync_string == "nosync_safe")
		{
			sync = nano::lmdb_config::sync_strategy::nosync_safe;
		}
		else if (sync_string == "nosync_unsafe")
		{
			sync = nano::lmdb_config::sync_strategy::nosync_unsafe;
		}
		else if (sync_string == "nosync_unsafe_large_memory")
		{
			sync = nano::lmdb_config::sync_strategy::nosync_unsafe_large_memory;
		}
		else
		{
			toml.get_error ().set (sync_string + " is not a valid sync option");
		}
	}

	return toml.get_error ();
}
