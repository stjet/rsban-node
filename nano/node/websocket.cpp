#include "nano/lib/rsnano.hpp"

#include <nano/boost/asio/bind_executor.hpp>
#include <nano/boost/asio/dispatch.hpp>
#include <nano/boost/asio/strand.hpp>
#include <nano/lib/blocks.hpp>
#include <nano/lib/logging.hpp>
#include <nano/lib/rsnanoutils.hpp>
#include <nano/lib/work.hpp>
#include <nano/node/election_status.hpp>
#include <nano/node/node_observers.hpp>
#include <nano/node/transport/channel.hpp>
#include <nano/node/transport/transport.hpp>
#include <nano/node/wallet.hpp>
#include <nano/node/websocket.hpp>
#include <nano/secure/ledger.hpp>

#include <boost/algorithm/string.hpp>
#include <boost/property_tree/json_parser.hpp>

#include <chrono>

nano::websocket::options::options () :
	handle{ rsnano::rsn_websocket_options_create () }
{
}
nano::websocket::options::options (rsnano::WebsocketOptionsHandle * handle) :
	handle{ handle }
{
}

nano::websocket::options::~options ()
{
	rsnano::rsn_websocket_options_destroy (handle);
}

void nano::websocket::listener::stop ()
{
	rsnano::rsn_websocket_listener_stop (handle);
}

nano::websocket::listener::listener (rsnano::async_runtime & async_rt, nano::wallets & wallets_a, boost::asio::io_context & io_ctx_a, boost::asio::ip::tcp::endpoint endpoint_a)
{
	auto endpoint_dto{ rsnano::endpoint_to_dto (endpoint_a) };
	handle = rsnano::rsn_websocket_listener_create (&endpoint_dto, wallets_a.rust_handle,
	async_rt.handle);
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

nano::websocket::message nano::websocket::message_builder::started_election (nano::block_hash const & hash_a)
{
	return { rsnano::rsn_message_builder_started_election (hash_a.bytes.data ()) };
}

nano::websocket::message nano::websocket::message_builder::stopped_election (nano::block_hash const & hash_a)
{
	return { rsnano::rsn_message_builder_stopped_election (hash_a.bytes.data ()) };
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

nano::websocket::message nano::websocket::message_builder::telemetry_received (nano::telemetry_data const & telemetry_data_a, nano::endpoint const & endpoint_a)
{
	auto endpoint_dto{ rsnano::udp_endpoint_to_dto (endpoint_a) };
	return { rsnano::rsn_message_builder_telemetry_received (telemetry_data_a.handle, &endpoint_dto) };
}

nano::websocket::message nano::websocket::message_builder::new_block_arrived (nano::block const & block_a)
{
	return { rsnano::rsn_message_builder_new_block_arrived (block_a.get_handle ()) };
}

/*
 * websocket_server
 */

nano::websocket_server::websocket_server (rsnano::async_runtime & async_rt, nano::websocket::config & config_a, nano::node_observers & observers_a, nano::wallets & wallets_a, nano::ledger & ledger_a, boost::asio::io_context & io_ctx_a, nano::logger & logger_a) :
	config{ config_a },
	observers{ observers_a },
	wallets{ wallets_a },
	ledger{ ledger_a },
	io_ctx{ io_ctx_a },
	logger{ logger_a }
{
	if (!config.enabled)
	{
		return;
	}

	auto endpoint = nano::tcp_endpoint{ boost::asio::ip::make_address_v6 (config.address), config.port };
	server = std::make_shared<nano::websocket::listener> (async_rt, wallets, io_ctx, endpoint);

	observers.blocks.add ([this] (nano::election_status const & status_a, std::vector<nano::vote_with_weight_info> const & votes_a, nano::account const & account_a, nano::amount const & amount_a, bool is_state_send_a, bool is_state_epoch_a) {
		debug_assert (status_a.get_election_status_type () != nano::election_status_type::ongoing);

		if (server->any_subscriber (nano::websocket::topic::confirmation))
		{
			auto block_a = status_a.get_winner ();
			std::string subtype;
			if (is_state_send_a)
			{
				subtype = "send";
			}
			else if (block_a->type () == nano::block_type::state)
			{
				if (block_a->is_change ())
				{
					subtype = "change";
				}
				else if (is_state_epoch_a)
				{
					debug_assert (amount_a == 0 && ledger.is_epoch_link (block_a->link_field ().value ()));
					subtype = "epoch";
				}
				else
				{
					subtype = "receive";
				}
			}

			server->broadcast_confirmation (block_a, account_a, amount_a, subtype, status_a, votes_a);
		}
	});

	observers.active_started.add ([this] (nano::block_hash const & hash_a) {
		if (server->any_subscriber (nano::websocket::topic::started_election))
		{
			nano::websocket::message_builder builder;
			server->broadcast (builder.started_election (hash_a));
		}
	});

	observers.active_stopped.add ([this] (nano::block_hash const & hash_a) {
		if (server->any_subscriber (nano::websocket::topic::stopped_election))
		{
			nano::websocket::message_builder builder;
			server->broadcast (builder.stopped_election (hash_a));
		}
	});

	observers.telemetry.add ([this] (nano::telemetry_data const & telemetry_data, std::shared_ptr<nano::transport::channel> const & channel) {
		if (server->any_subscriber (nano::websocket::topic::telemetry))
		{
			nano::websocket::message_builder builder;
			server->broadcast (builder.telemetry_received (telemetry_data, channel->get_remote_endpoint ()));
		}
	});

	observers.vote.add ([this] (std::shared_ptr<nano::vote> vote_a, std::shared_ptr<nano::transport::channel> const & channel_a, nano::vote_code code_a) {
		if (server->any_subscriber (nano::websocket::topic::vote))
		{
			nano::websocket::message_builder builder;
			auto msg{ builder.vote_received (vote_a, code_a) };
			server->broadcast (msg);
		}
	});
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
