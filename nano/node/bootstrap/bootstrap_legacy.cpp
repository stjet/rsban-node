#include "nano/lib/rsnano.hpp"

#include <nano/node/bootstrap/bootstrap_bulk_push.hpp>
#include <nano/node/bootstrap/bootstrap_frontier.hpp>
#include <nano/node/bootstrap/bootstrap_legacy.hpp>
#include <nano/node/node.hpp>

#include <boost/format.hpp>

namespace
{
rsnano::BootstrapAttemptHandle * create_legacy_handle (
nano::bootstrap_attempt_legacy * this_l,
std::shared_ptr<nano::node> const & node_a,
uint64_t const incremental_id_a,
std::string const & id_a,
uint32_t const frontiers_age_a,
nano::account const & start_account_a)
{
	auto params_dto{ node_a->network_params.to_dto () };
	auto config_dto{ node_a->config->to_dto () };
	return rsnano::rsn_bootstrap_attempt_legacy_create (
	node_a->websocket.server != nullptr ? node_a->websocket.server->handle : nullptr,
	node_a->block_processor.handle,
	node_a->bootstrap_initiator.get_handle (),
	node_a->ledger.handle,
	id_a.c_str (),
	incremental_id_a,
	node_a->bootstrap_initiator.connections->handle,
	&params_dto,
	&config_dto,
	node_a->flags.handle,
	node_a->stats->handle,
	frontiers_age_a,
	start_account_a.bytes.data ());
}
}

nano::bootstrap_attempt_legacy::bootstrap_attempt_legacy (std::shared_ptr<nano::node> const & node_a, uint64_t const incremental_id_a, std::string const & id_a, uint32_t const frontiers_age_a, nano::account const & start_account_a) :
	nano::bootstrap_attempt (create_legacy_handle (this, node_a, incremental_id_a, id_a, frontiers_age_a, start_account_a))
{
}

void nano::bootstrap_attempt_legacy::add_frontier (nano::pull_info const & pull_a)
{
	auto dto{ pull_a.to_dto () };
	rsnano::rsn_bootstrap_attempt_legacy_add_frontier (handle, &dto);
}

void nano::bootstrap_attempt_legacy::add_bulk_push_target (nano::block_hash const & head, nano::block_hash const & end)
{
	rsnano::rsn_bootstrap_attempt_legacy_add_bulk_push_target (handle, head.bytes.data (), end.bytes.data ());
}

bool nano::bootstrap_attempt_legacy::request_bulk_push_target (std::pair<nano::block_hash, nano::block_hash> & current_target_a)
{
	return rsnano::rsn_bootstrap_attempt_legacy_request_bulk_push_target (handle, current_target_a.first.bytes.data (), current_target_a.second.bytes.data ());
}

void nano::bootstrap_attempt_legacy::set_start_account (nano::account const & start_account_a)
{
	rsnano::rsn_bootstrap_attempt_legacy_set_start_account (handle, start_account_a.bytes.data ());
}

void nano::bootstrap_attempt_legacy::get_information (boost::property_tree::ptree & tree_a)
{
	rsnano::rsn_bootstrap_attempt_legacy_get_information (handle, (void *)&tree_a);
}
