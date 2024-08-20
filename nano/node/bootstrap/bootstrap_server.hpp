#pragma once

#include "nano/lib/rsnano.hpp"

#include <nano/lib/observer_set.hpp>
#include <nano/node/messages.hpp>

namespace nano
{
class bootstrap_server_config final
{
public:
	nano::error deserialize (nano::tomlconfig &);
	void load_dto (rsnano::BootstrapServerConfigDto const & dto);
	rsnano::BootstrapServerConfigDto to_dto () const;

public:
	size_t max_queue{ 16 };
	size_t threads{ 1 };
	size_t batch_size{ 64 };
};
}
