#include "nano/lib/rsnano.hpp"
#include "nano/lib/rsnanoutils.hpp"

#include <nano/lib/blocks.hpp>
#include <nano/node/bootstrap/bootstrap.hpp>
#include <nano/node/bootstrap/bootstrap_lazy.hpp>
#include <nano/node/common.hpp>
#include <nano/node/node.hpp>
#include <nano/secure/ledger.hpp>

#include <boost/format.hpp>

constexpr std::chrono::seconds nano::bootstrap_limits::lazy_flush_delay_sec;
constexpr uint64_t nano::bootstrap_limits::lazy_batch_pull_count_resize_blocks_limit;
constexpr double nano::bootstrap_limits::lazy_batch_pull_count_resize_ratio;
constexpr std::size_t nano::bootstrap_limits::lazy_blocks_restart_limit;

namespace
{
rsnano::BootstrapAttemptHandle * create_lazy_handle (std::shared_ptr<nano::node> const & node_a, uint64_t incremental_id_a, std::string const & id_a)
{
	auto network_params_dto{ node_a->network_params.to_dto () };
	return rsnano::rsn_bootstrap_attempt_lazy_create (
	node_a->websocket.server != nullptr ? node_a->websocket.server->handle : nullptr,
	node_a->block_processor.get_handle (),
	node_a->bootstrap_initiator.get_handle (),
	node_a->ledger.get_handle (),
	id_a.c_str (),
	incremental_id_a,
	node_a->flags.handle,
	node_a->bootstrap_initiator.connections->handle,
	&network_params_dto);
}

rsnano::BootstrapAttemptHandle * create_wallet_handle (std::shared_ptr<nano::node> const & node_a, uint64_t incremental_id_a, std::string const & id_a)
{
	auto config_dto{ node_a->config->to_dto () };

	return rsnano::rsn_bootstrap_attempt_wallet_create (
	node_a->websocket.server != nullptr ? node_a->websocket.server->handle : nullptr,
	node_a->block_processor.get_handle (),
	node_a->bootstrap_initiator.get_handle (),
	node_a->ledger.get_handle (),
	id_a.c_str (),
	incremental_id_a,
	node_a->bootstrap_initiator.connections->handle,
	node_a->workers->handle,
	&config_dto,
	node_a->stats->handle);
}
}

nano::bootstrap_attempt_lazy::bootstrap_attempt_lazy (std::shared_ptr<nano::node> const & node_a, uint64_t incremental_id_a, std::string const & id_a) :
	nano::bootstrap_attempt (create_lazy_handle (node_a, incremental_id_a, id_a))
{
}

nano::bootstrap_attempt_lazy::bootstrap_attempt_lazy (rsnano::BootstrapAttemptHandle * handle) :
	nano::bootstrap_attempt{ handle }
{
}

void nano::bootstrap_attempt_lazy::get_information (boost::property_tree::ptree & tree_a)
{
	rsnano::rsn_bootstrap_attempt_lazy_get_information (handle, &tree_a);
}

nano::bootstrap_attempt_wallet::bootstrap_attempt_wallet (rsnano::BootstrapAttemptHandle * handle) :
	nano::bootstrap_attempt{ handle }
{
}

nano::bootstrap_attempt_wallet::bootstrap_attempt_wallet (std::shared_ptr<nano::node> const & node_a, uint64_t incremental_id_a, std::string id_a) :
	nano::bootstrap_attempt (create_wallet_handle (node_a, incremental_id_a, id_a))
{
}

std::size_t nano::bootstrap_attempt_wallet::wallet_size ()
{
	return rsnano::rsn_bootstrap_attempt_wallet_size (handle);
}

void nano::bootstrap_attempt_wallet::get_information (boost::property_tree::ptree & tree_a)
{
	tree_a.put ("wallet_accounts", std::to_string (wallet_size ()));
}
