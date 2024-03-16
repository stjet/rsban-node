#pragma once

#include "nano/lib/numbers.hpp"

#include <nano/node/common.hpp>

#include <deque>
#include <future>
#include <memory>

namespace rsnano
{
class FrontierReqClientHandle;
}

namespace nano
{
class bootstrap_attempt_legacy;
class bootstrap_client;
class node;
namespace transport
{
	class tcp_server;
}

/**
 * Client side of a frontier request. Created to send and listen for frontier sequences from the server.
 */
class frontier_req_client final : public std::enable_shared_from_this<nano::frontier_req_client>
{
public:
	explicit frontier_req_client (std::shared_ptr<nano::node> const &, std::shared_ptr<nano::bootstrap_client> const &, std::shared_ptr<nano::bootstrap_attempt_legacy> const &);
	frontier_req_client (frontier_req_client const &) = delete;
	~frontier_req_client ();
	void run (nano::account const & start_account_a, uint32_t const frontiers_age_a, uint32_t const count_a);
	bool get_result ();
	void set_result (bool value);
	rsnano::FrontierReqClientHandle * handle;
};

class frontier_req;

/**
 * Server side of a frontier request. Created when a tcp_server receives a frontier_req message and exited when end-of-list is reached.
 */
class frontier_req_server final : public std::enable_shared_from_this<nano::frontier_req_server>
{
public:
	frontier_req_server (std::shared_ptr<nano::node> const &, std::shared_ptr<nano::transport::tcp_server> const &, std::unique_ptr<nano::frontier_req>);
	frontier_req_server (frontier_req_server const &) = delete;
	~frontier_req_server ();
	void send_next ();
	nano::public_key current () const;
	nano::block_hash frontier () const;
	rsnano::FrontierReqServerHandle * handle;
};
}
