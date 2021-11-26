#include <nano/lib/config.hpp>
#include <nano/lib/rocksdbconfig.hpp>
#include <nano/lib/tomlconfig.hpp>

nano::rocksdb_config::rocksdb_config ()
{
	rsnano::RocksDbConfigDto dto;
	rsnano::rsn_rocksdb_config_create (&dto);
	load_dto (dto);
}

void nano::rocksdb_config::load_dto (rsnano::RocksDbConfigDto & dto)
{
	enable = dto.enable;
	memory_multiplier = dto.memory_multiplier;
	io_threads = dto.io_threads;
}

rsnano::RocksDbConfigDto nano::rocksdb_config::to_dto () const
{
	rsnano::RocksDbConfigDto dto;
	dto.enable = enable;
	dto.memory_multiplier = memory_multiplier;
	dto.io_threads = io_threads;
	return dto;
}

nano::error nano::rocksdb_config::deserialize_toml (nano::tomlconfig & toml)
{
	toml.get_optional<bool> ("enable", enable);
	toml.get_optional<uint8_t> ("memory_multiplier", memory_multiplier);
	toml.get_optional<unsigned> ("io_threads", io_threads);

	// Validate ranges
	if (io_threads == 0)
	{
		toml.get_error ().set ("io_threads must be non-zero");
	}
	if (memory_multiplier < 1 || memory_multiplier > 3)
	{
		toml.get_error ().set ("memory_multiplier must be either 1, 2 or 3");
	}

	return toml.get_error ();
}

bool nano::rocksdb_config::using_rocksdb_in_tests ()
{
	return rsnano::rsn_using_rocksdb_in_tests ();
}
