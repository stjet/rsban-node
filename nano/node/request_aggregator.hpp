#pragma once

#include "nano/lib/rsnano.hpp"

#include <nano/lib/locks.hpp>
#include <nano/lib/numbers.hpp>
#include <nano/node/transport/transport.hpp>

namespace nano
{
class request_aggregator_config final
{
public:
	request_aggregator_config () = default;
	explicit request_aggregator_config (rsnano::RequestAggregatorConfigDto const & dto);

	rsnano::RequestAggregatorConfigDto into_dto () const;
	nano::error deserialize (nano::tomlconfig &);

public:
	size_t threads;
	size_t max_queue;
	size_t batch_size;
};
}
