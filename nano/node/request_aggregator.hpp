#pragma once

#include "nano/lib/rsnano.hpp"

#include <nano/lib/locks.hpp>
#include <nano/lib/numbers.hpp>
#include <nano/node/transport/channel.hpp>
#include <nano/node/transport/transport.hpp>

#include <vector>

namespace nano
{
class request_aggregator_config final
{
public:
	request_aggregator_config() = default;
	explicit request_aggregator_config(rsnano::RequestAggregatorConfigDto const & dto);

	rsnano::RequestAggregatorConfigDto into_dto() const;
	nano::error deserialize (nano::tomlconfig &);

public:
	size_t threads;
	size_t max_queue;
	size_t batch_size;
};
/**
 * Pools together confirmation requests, separately for each endpoint.
 * Requests are added from network messages, and aggregated to minimize bandwidth and vote generation. Example:
 * * Two votes are cached, one for hashes {1,2,3} and another for hashes {4,5,6}
 * * A request arrives for hashes {1,4,5}. Another request arrives soon afterwards for hashes {2,3,6}
 * * The aggregator will reply with the two cached votes
 * Votes are generated for uncached hashes.
 */
class request_aggregator final
{
public:
	request_aggregator (rsnano::RequestAggregatorHandle * handle);
	request_aggregator (request_aggregator const &) = delete;
	~request_aggregator ();

	/** Add a new request by \p channel_a for hashes \p hashes_roots_a */
	void add (std::shared_ptr<nano::transport::channel> const & channel_a, std::vector<std::pair<nano::block_hash, nano::root>> const & hashes_roots_a);
	/** Returns the number of currently queued request pools */
	std::size_t size ();
	bool empty ();
	std::chrono::milliseconds get_max_delay () const;

	rsnano::RequestAggregatorHandle * handle;
};
}
