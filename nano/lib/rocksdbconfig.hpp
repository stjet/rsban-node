#pragma once

#include <nano/lib/errors.hpp>
#include <nano/lib/rsnano.hpp>

#include <thread>

namespace nano
{
class tomlconfig;

/** Configuration options for RocksDB */
class rocksdb_config final
{
public:
	rocksdb_config ();
	void load_dto (rsnano::RocksDbConfigDto & dto);
	rsnano::RocksDbConfigDto to_dto () const;
	nano::error deserialize_toml (nano::tomlconfig & toml_a);

	/** To use RocksDB in tests make sure the environment variable TEST_USE_ROCKSDB=1 is set */
	static bool using_rocksdb_in_tests ();

	bool enable;
	uint8_t memory_multiplier;
	unsigned io_threads;
};
}
