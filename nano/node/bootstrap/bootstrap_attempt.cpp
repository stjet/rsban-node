#include "nano/lib/rsnano.hpp"

#include <nano/lib/blocks.hpp>
#include <nano/lib/rsnanoutils.hpp>
#include <nano/node/bootstrap/bootstrap.hpp>
#include <nano/node/bootstrap/bootstrap_attempt.hpp>
#include <nano/node/node.hpp>
#include <nano/node/websocket.hpp>
#include <nano/secure/ledger.hpp>

#include <boost/format.hpp>

constexpr unsigned nano::bootstrap_limits::requeued_pulls_limit;
constexpr unsigned nano::bootstrap_limits::requeued_pulls_limit_dev;

nano::bootstrap_attempt::bootstrap_attempt (std::shared_ptr<nano::node> const & node_a, nano::bootstrap_mode mode_a, uint64_t incremental_id_a, std::string id_a)
{
	handle = rsnano::rsn_bootstrap_attempt_create (
	node_a->websocket.server != nullptr ? node_a->websocket.server->handle : nullptr,
	node_a->block_processor.get_handle (),
	node_a->bootstrap_initiator.get_handle (),
	node_a->ledger.get_handle (),
	id_a.c_str (),
	static_cast<uint8_t> (mode_a),
	incremental_id_a);
}

nano::bootstrap_attempt::bootstrap_attempt (rsnano::BootstrapAttemptHandle * handle_a) :
	handle{ handle_a }
{
}

nano::bootstrap_attempt::~bootstrap_attempt ()
{
	rsnano::rsn_bootstrap_attempt_destroy (handle);
}

std::string nano::bootstrap_attempt::id () const
{
	rsnano::StringDto str_result;
	rsnano::rsn_bootstrap_attempt_id (handle, &str_result);
	return rsnano::convert_dto_to_string (str_result);
}

uint64_t nano::bootstrap_attempt::total_blocks () const
{
	return rsnano::rsn_bootstrap_attempt_total_blocks (handle);
}

void nano::bootstrap_attempt::total_blocks_inc ()
{
	rsnano::rsn_bootstrap_attempt_total_blocks_inc (handle);
}

unsigned nano::bootstrap_attempt::get_pulling () const
{
	return rsnano::rsn_bootstrap_attempt_pulling (handle);
}

void nano::bootstrap_attempt::inc_pulling ()
{
	rsnano::rsn_bootstrap_attempt_pulling_inc (handle);
}

bool nano::bootstrap_attempt::get_started () const
{
	return rsnano::rsn_bootstrap_attempt_started (handle);
}

unsigned nano::bootstrap_attempt::get_requeued_pulls () const
{
	return rsnano::rsn_bootstrap_attempt_requeued_pulls (handle);
}

bool nano::bootstrap_attempt::get_stopped () const
{
	return rsnano::rsn_bootstrap_attempt_stopped (handle);
}

