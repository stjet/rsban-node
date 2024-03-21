#include "nano/lib/logging.hpp"
#include "nano/lib/numbers.hpp"
#include "nano/lib/rsnano.hpp"

#include <nano/node/bootstrap/bootstrap_attempt.hpp>
#include <nano/node/bootstrap/bootstrap_frontier.hpp>
#include <nano/node/bootstrap/bootstrap_legacy.hpp>
#include <nano/node/node.hpp>
#include <nano/node/transport/tcp.hpp>
#include <nano/secure/ledger.hpp>

#include <boost/format.hpp>

constexpr double nano::bootstrap_limits::bootstrap_connection_warmup_time_sec;
constexpr double nano::bootstrap_limits::bootstrap_minimum_elapsed_seconds_blockrate;
constexpr double nano::bootstrap_limits::bootstrap_minimum_frontier_blocks_per_sec;
constexpr unsigned nano::bootstrap_limits::bulk_push_cost_limit;

namespace
{
rsnano::FrontierReqClientHandle * create_client_handle (
std::shared_ptr<nano::node> const & node_a,
std::shared_ptr<nano::bootstrap_client> const & connection_a,
std::shared_ptr<nano::bootstrap_attempt_legacy> const & attempt_a)
{
	auto network_params_dto{ node_a->network_params.to_dto () };
	return rsnano::rsn_frontier_req_client_create (
	connection_a->handle,
	node_a->ledger.handle,
	&network_params_dto,
	node_a->bootstrap_initiator.connections->handle,
	attempt_a->handle);
}
}

nano::frontier_req_client::frontier_req_client (std::shared_ptr<nano::node> const & node_a, std::shared_ptr<nano::bootstrap_client> const & connection_a, std::shared_ptr<nano::bootstrap_attempt_legacy> const & attempt_a) :
	handle{ create_client_handle (node_a, connection_a, attempt_a) }
{
}

bool nano::frontier_req_client::get_result ()
{
	return rsnano::rsn_frontier_req_client_get_result (handle);
}

void nano::frontier_req_client::set_result (bool value)
{
	rsnano::rsn_frontier_req_client_set_result (handle, value);
}

void nano::frontier_req_client::run (nano::account const & start_account_a, uint32_t const frontiers_age_a, uint32_t const count_a)
{
	rsnano::rsn_frontier_req_client_run (handle, start_account_a.bytes.data (), frontiers_age_a, count_a);
}

nano::frontier_req_client::~frontier_req_client ()
{
	rsnano::rsn_frontier_req_client_destroy (handle);
}

//------------------------------------------------------------------------------
// frontier_req_server
//------------------------------------------------------------------------------

namespace
{
rsnano::FrontierReqServerHandle * create_frontier_req_server_handle (
std::shared_ptr<nano::node> const & node_a,
std::shared_ptr<nano::transport::tcp_server> const & connection_a,
rsnano::MessageHandle * request_a)
{
	return rsnano::rsn_frontier_req_server_create (connection_a->handle,
	request_a,
	node_a->bootstrap_workers->handle,
	node_a->ledger.get_handle ());
}
}

nano::frontier_req_server::frontier_req_server (std::shared_ptr<nano::node> const & node_a, std::shared_ptr<nano::transport::tcp_server> const & connection_a, std::unique_ptr<nano::frontier_req> request_a) :
	handle{ create_frontier_req_server_handle (node_a, connection_a, request_a->handle) }
{
}

nano::frontier_req_server::~frontier_req_server ()
{
	rsnano::rsn_frontier_req_server_destroy (handle);
}

void nano::frontier_req_server::send_next ()
{
	rsnano::rsn_frontier_req_server_send_next (handle);
}

nano::public_key nano::frontier_req_server::current () const
{
	nano::public_key result;
	rsnano::rsn_frontier_req_server_current (handle, result.bytes.data ());
	return result;
}

nano::block_hash nano::frontier_req_server::frontier () const
{
	nano::block_hash result;
	rsnano::rsn_frontier_req_server_frontier (handle, result.bytes.data ());
	return result;
}
