#pragma once

#include "nano/lib/rsnano.hpp"

#include <nano/lib/observer_set.hpp>
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

/**
 * Processes bootstrap requests (`asc_pull_req` messages) and replies with bootstrap responses (`asc_pull_ack`)
 */
class bootstrap_server final
{
public:
	// `asc_pull_req` message is small, store by value
	using request_t = std::pair<nano::asc_pull_req, std::shared_ptr<nano::transport::channel>>; // <request, response channel>

public:
	bootstrap_server (rsnano::BootstrapServerHandle * handle);
	bootstrap_server (bootstrap_server const &) = delete;
	~bootstrap_server ();

	void set_response_callback (std::function<void (nano::asc_pull_ack const &, std::shared_ptr<nano::transport::channel> &)> callback);

	rsnano::BootstrapServerHandle * handle;

public: // Config
	/** Maximum number of blocks to send in a single response, cannot be higher than capacity of a single `asc_pull_ack` message */
	constexpr static std::size_t max_blocks = nano::asc_pull_ack::blocks_payload::max_blocks;
	constexpr static std::size_t max_frontiers = nano::asc_pull_ack::frontiers_payload::max_frontiers;
};
}
