#include "nano/lib/rsnano.hpp"

#include <nano/boost/asio/bind_executor.hpp>
#include <nano/boost/asio/dispatch.hpp>
#include <nano/boost/asio/strand.hpp>
#include <nano/lib/blocks.hpp>
#include <nano/lib/logging.hpp>
#include <nano/lib/rsnanoutils.hpp>
#include <nano/lib/work.hpp>
#include <nano/node/active_transactions.hpp>
#include <nano/node/election_status.hpp>
#include <nano/node/node_observers.hpp>
#include <nano/node/telemetry.hpp>
#include <nano/node/transport/channel.hpp>
#include <nano/node/transport/transport.hpp>
#include <nano/node/vote_processor.hpp>
#include <nano/node/wallet.hpp>
#include <nano/node/websocket.hpp>
#include <nano/secure/ledger.hpp>

#include <boost/algorithm/string.hpp>
#include <boost/property_tree/json_parser.hpp>

#include <chrono>
#include <memory>

void nano::websocket::listener::stop ()
{
	rsnano::rsn_websocket_listener_stop (handle);
}

nano::websocket::listener::~listener ()
{
	rsnano::rsn_websocket_listener_destroy (handle);
}

void nano::websocket::listener::run ()
{
	rsnano::rsn_websocket_listener_run (handle);
}

void nano::websocket::listener::broadcast_confirmation (std::shared_ptr<nano::block> const & block_a, nano::account const & account_a, nano::amount const & amount_a, std::string const & subtype, nano::election_status const & election_status_a, std::vector<nano::vote_with_weight_info> const & election_votes_a)
{
	auto vec_handle = rsnano::rsn_vote_with_weight_info_vec_create ();
	for (const auto & info : election_votes_a)
	{
		auto dto{ info.into_dto () };
		rsnano::rsn_vote_with_weight_info_vec_push (vec_handle, &dto);
	}
	rsnano::rsn_websocket_listener_broadcast_confirmation (
	handle,
	block_a->get_handle (),
	account_a.bytes.data (),
	amount_a.bytes.data (),
	subtype.c_str (),
	election_status_a.handle,
	vec_handle);
	rsnano::rsn_vote_with_weight_info_vec_destroy (vec_handle);
}

void nano::websocket::listener::broadcast (nano::websocket::message message_a)
{
	rsnano::rsn_websocket_listener_broadcast (handle, message_a.handle);
}

std::uint16_t nano::websocket::listener::listening_port ()
{
	return rsnano::rsn_websocket_listener_listening_port (handle);
}

std::size_t nano::websocket::listener::subscriber_count (nano::websocket::topic const & topic_a) const
{
	return rsnano::rsn_websocket_listener_subscriber_count (handle, static_cast<uint8_t> (topic_a));
}

nano::websocket::message nano::websocket::message_builder::vote_received (std::shared_ptr<nano::vote> const & vote_a, nano::vote_code code_a)
{
	return { rsnano::rsn_message_builder_vote_received (vote_a->get_handle (), static_cast<uint8_t> (code_a)) };
}

nano::websocket::message nano::websocket::message_builder::work_generation (nano::work_version const version_a, nano::block_hash const & root_a, uint64_t work_a, uint64_t difficulty_a, uint64_t publish_threshold_a, std::chrono::milliseconds const & duration_a, std::string const & peer_a, std::vector<std::string> const & bad_peers_a, bool completed_a, bool cancelled_a)
{
	rsnano::string_vec bad_peers_vec (bad_peers_a);
	auto msg_handle = rsnano::rsn_message_builder_work_generation (
	static_cast<uint8_t> (version_a),
	root_a.bytes.data (),
	work_a, difficulty_a, publish_threshold_a, duration_a.count (),
	peer_a.c_str (), bad_peers_vec.handle, completed_a, cancelled_a);
	return { msg_handle };
}

nano::websocket::message nano::websocket::message_builder::work_cancelled (nano::work_version const version_a, nano::block_hash const & root_a, uint64_t const difficulty_a, uint64_t const publish_threshold_a, std::chrono::milliseconds const & duration_a, std::vector<std::string> const & bad_peers_a)
{
	return work_generation (version_a, root_a, 0, difficulty_a, publish_threshold_a, duration_a, "", bad_peers_a, false, true);
}

nano::websocket::message nano::websocket::message_builder::work_failed (nano::work_version const version_a, nano::block_hash const & root_a, uint64_t const difficulty_a, uint64_t const publish_threshold_a, std::chrono::milliseconds const & duration_a, std::vector<std::string> const & bad_peers_a)
{
	return work_generation (version_a, root_a, 0, difficulty_a, publish_threshold_a, duration_a, "", bad_peers_a, false, false);
}

/*
 * websocket_server
 */

nano::websocket_server::websocket_server (rsnano::async_runtime & async_rt, nano::websocket::config & config_a, nano::wallets & wallets_a, nano::active_transactions & active_transactions_a, nano::telemetry & telemetry_a, nano::vote_processor & vote_processor_a)
{
	auto config_dto{ config_a.to_dto () };
	auto listener_handle = rsnano::rsn_websocket_server_create (&config_dto, wallets_a.rust_handle, async_rt.handle,
	active_transactions_a.handle, telemetry_a.handle, vote_processor_a.handle);

	if (listener_handle != nullptr)
	{
		server = std::make_unique<nano::websocket::listener> (listener_handle);
	}
}

nano::websocket_server::websocket_server (rsnano::WebsocketListenerHandle * handle)
{
	if (handle != nullptr)
	{
		server = std::make_unique<nano::websocket::listener> (handle);
	}
}

void nano::websocket_server::start ()
{
	if (server)
	{
		server->run ();
	}
}

void nano::websocket_server::stop ()
{
	if (server)
	{
		server->stop ();
	}
}

rsnano::WebsocketListenerHandle * nano::websocket_server::get_handle ()
{
	if (server)
	{
		return server->handle;
	}

	return nullptr;
}
