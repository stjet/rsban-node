#pragma once

#include <nano/lib/observer_set.hpp>
#include <nano/lib/processing_queue.hpp>
#include <nano/node/messages.hpp>

#include <memory>
#include <utility>

namespace nano::store
{
class transaction;
class component;
}

namespace nano
{
class ledger;
namespace transport
{
	class channel;
}

/**
 * Processes bootstrap requests (`asc_pull_req` messages) and replies with bootstrap responses (`asc_pull_ack`)
 *
 * In order to ensure maximum throughput, there are two internal processing queues:
 * - One for doing ledger lookups and preparing responses (`request_queue`)
 * - One for sending back those responses over the network (`response_queue`)
 */
class bootstrap_server final
{
public:
	// `asc_pull_req` message is small, store by value
	using request_t = std::pair<nano::asc_pull_req, std::shared_ptr<nano::transport::channel>>; // <request, response channel>

public:
	bootstrap_server (nano::store::component &, nano::ledger &, nano::network_constants const &, nano::stats &);
	bootstrap_server (bootstrap_server const &) = delete;
	~bootstrap_server ();

	void start ();
	void stop ();

	/**
	 * Process `asc_pull_req` message coming from network.
	 * Reply will be sent back over passed in `channel`
	 */
	bool request (nano::asc_pull_req const & message, std::shared_ptr<nano::transport::channel> channel);

	void set_response_callback (std::function<void (nano::asc_pull_ack &, std::shared_ptr<nano::transport::channel> &)> callback);

	rsnano::BootstrapServerHandle * handle;

public: // Config
	/** Maximum number of blocks to send in a single response, cannot be higher than capacity of a single `asc_pull_ack` message */
	constexpr static std::size_t max_blocks = nano::asc_pull_ack::blocks_payload::max_blocks;
	constexpr static std::size_t max_frontiers = nano::asc_pull_ack::frontiers_payload::max_frontiers;
};
}
