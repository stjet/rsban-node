#include "nano/lib/rsnano.hpp"

#include <nano/lib/blocks.hpp>
#include <nano/lib/stats.hpp>
#include <nano/lib/threading.hpp>
#include <nano/node/active_elections.hpp>
#include <nano/node/common.hpp>
#include <nano/node/local_vote_history.hpp>
#include <nano/node/network.hpp>
#include <nano/node/nodeconfig.hpp>
#include <nano/node/request_aggregator.hpp>
#include <nano/node/wallet.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/store/component.hpp>

/*
 * request_aggregator_config
 */

nano::request_aggregator_config::request_aggregator_config (rsnano::RequestAggregatorConfigDto const & dto) :
	max_queue{ dto.max_queue },
	threads{ dto.threads },
	batch_size{ dto.batch_size }
{
}

rsnano::RequestAggregatorConfigDto nano::request_aggregator_config::into_dto () const
{
	return { threads, max_queue, batch_size };
}

nano::error nano::request_aggregator_config::deserialize (nano::tomlconfig & toml)
{
	toml.get ("max_queue", max_queue);
	toml.get ("threads", threads);
	toml.get ("batch_size", batch_size);

	return toml.get_error ();
}
