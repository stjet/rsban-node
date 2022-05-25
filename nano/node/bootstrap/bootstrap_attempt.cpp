#include <nano/crypto_lib/random_pool.hpp>
#include <nano/node/bootstrap/bootstrap.hpp>
#include <nano/node/bootstrap/bootstrap_attempt.hpp>
#include <nano/node/bootstrap/bootstrap_bulk_push.hpp>
#include <nano/node/bootstrap/bootstrap_frontier.hpp>
#include <nano/node/common.hpp>
#include <nano/node/node.hpp>
#include <nano/node/transport/tcp.hpp>
#include <nano/node/websocket.hpp>

#include <boost/format.hpp>

#include <algorithm>

constexpr unsigned nano::bootstrap_limits::requeued_pulls_limit;
constexpr unsigned nano::bootstrap_limits::requeued_pulls_limit_dev;

nano::bootstrap_attempt::bootstrap_attempt (std::shared_ptr<nano::node> const & node_a, nano::bootstrap_mode mode_a, uint64_t incremental_id_a, std::string id_a) :
	handle (rsnano::rsn_bootstrap_attempt_create (&node_a->logger, node_a->websocket_server.get (), node_a->block_processor.get_handle (), node_a->bootstrap_initiator.get_handle (), node_a->ledger.get_handle (), id_a.c_str (), static_cast<uint8_t> (mode_a), incremental_id_a)),
	node (node_a),
	mode (mode_a)
{
}

std::string nano::bootstrap_attempt::id () const
{
	rsnano::StringDto str_result;
	rsnano::rsn_bootstrap_attempt_id (handle, &str_result);
	std::string id (str_result.value);
	rsnano::rsn_string_destroy (str_result.handle);
	return id;
}

uint64_t nano::bootstrap_attempt::get_incremental_id () const
{
	return rsnano::rsn_bootstrap_attempt_incremental_id (handle);
}

nano::bootstrap_attempt::~bootstrap_attempt ()
{
	rsnano::rsn_bootstrap_attempt_destroy (handle);
}

bool nano::bootstrap_attempt::should_log ()
{
	return rsnano::rsn_bootstrap_attempt_should_log (handle);
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

bool nano::bootstrap_attempt::get_stopped () const
{
	return rsnano::rsn_bootstrap_attempt_stopped (handle);
}

void nano::bootstrap_attempt::set_stopped ()
{
	rsnano::rsn_bootstrap_attempt_set_stopped (handle);
}

bool nano::bootstrap_attempt::still_pulling ()
{
	return rsnano::rsn_bootstrap_attempt_still_pulling (handle);
}

void nano::bootstrap_attempt::pull_started ()
{
	rsnano::rsn_bootstrap_attempt_pull_started (handle);
}

void nano::bootstrap_attempt::pull_finished ()
{
	rsnano::rsn_bootstrap_attempt_pull_finished (handle);
}

void nano::bootstrap_attempt::stop ()
{
	rsnano::rsn_bootstrap_attempt_stop (handle);
}

void nano::bootstrap_attempt::notify_all ()
{
	rsnano::rsn_bootstrap_attempt_notifiy_all (handle);
}

std::string nano::bootstrap_attempt::mode_text ()
{
	std::size_t len;
	auto ptr{ rsnano::rsn_bootstrap_attempt_bootstrap_mode (handle, &len) };
	std::string mode_text (ptr, len);
	return mode_text;
}

bool nano::bootstrap_attempt::process_block (std::shared_ptr<nano::block> const & block_a, nano::account const & known_account_a, uint64_t pull_blocks_processed, nano::bulk_pull::count_t max_blocks, bool block_expected, unsigned retry_limit)
{
	return rsnano::rsn_bootstrap_attempt_process_block (handle, block_a->get_handle (), known_account_a.bytes.data (), pull_blocks_processed, max_blocks, block_expected, retry_limit);
}
