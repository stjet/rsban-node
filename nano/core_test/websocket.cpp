#include <nano/core_test/fakes/websocket_client.hpp>
#include <nano/lib/blocks.hpp>
#include <nano/node/active_elections.hpp>
#include <nano/node/websocket.hpp>
#include <nano/test_common/network.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/telemetry.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

#include <boost/property_tree/json_parser.hpp>

#include <chrono>
#include <cstdlib>
#include <future>
#include <memory>
#include <sstream>
#include <string>
#include <thread>
#include <vector>

using namespace std::chrono_literals;

// Tests sending keepalive
TEST (websocket, ws_keepalive)
{
	nano::test::system system;
	nano::node_config config = system.default_config ();
	config.websocket_config.enabled = true;
	config.websocket_config.port = system.get_available_port ();
	auto node1 (system.add_node (config));

	auto task = ([&node1] () {
		fake_websocket_client client (node1->websocket.server->listening_port ());
		client.send_message (R"json({"action": "ping"})json");
		client.await_ack ();
	});
	auto future = std::async (std::launch::async, task);

	ASSERT_TIMELY_EQ (5s, future.wait_for (0s), std::future_status::ready);
}

// Tests sending telemetry
TEST (websocket, telemetry)
{
	nano::test::system system;
	nano::node_config config = system.default_config ();
	config.websocket_config.enabled = true;
	config.websocket_config.port = system.get_available_port ();
	nano::node_flags node_flags;
	auto node1 (system.add_node (config, node_flags));
	config.peering_port = system.get_available_port ();
	config.websocket_config.enabled = true;
	config.websocket_config.port = system.get_available_port ();
	auto node2 (system.add_node (config, node_flags));

	nano::test::wait_peer_connections (system);

	std::atomic<bool> done{ false };
	auto task = ([config = node1->config, &node1, &done] () {
		fake_websocket_client client (node1->websocket.server->listening_port ());
		client.send_message (R"json({"action": "subscribe", "topic": "telemetry", "ack": true})json");
		client.await_ack ();
		done = true;
		EXPECT_EQ (1, node1->websocket.server->subscriber_count (nano::websocket::topic::telemetry));
		return client.get_response ();
	});

	auto future = std::async (std::launch::async, task);

	ASSERT_TIMELY (10s, done);

	auto remote = node1->find_endpoint_for_node_id (node2->get_node_id ());
	ASSERT_TRUE (remote.has_value ());
	ASSERT_TIMELY (5s, node1->telemetry->get_telemetry (remote.value ()));

	ASSERT_TIMELY_EQ (10s, future.wait_for (0s), std::future_status::ready);

	// Check the telemetry notification message
	auto response = future.get ();

	std::stringstream stream;
	stream << response;
	boost::property_tree::ptree event;
	boost::property_tree::read_json (stream, event);
	ASSERT_EQ (event.get<std::string> ("topic"), "telemetry");

	auto & contents = event.get_child ("message");
	nano::jsonconfig telemetry_contents (contents);
	nano::telemetry_data telemetry_data;
	telemetry_data.deserialize_json (telemetry_contents, false);

	ASSERT_TRUE (nano::test::compare_telemetry (telemetry_data, *node2));

	ASSERT_EQ (contents.get<std::string> ("address"), remote.value ().address ().to_string ());
	ASSERT_EQ (contents.get<uint16_t> ("port"), remote.value ().port ());

	// Other node should have no subscribers
	EXPECT_EQ (0, node2->websocket.server->subscriber_count (nano::websocket::topic::telemetry));
}

TEST (websocket, new_unconfirmed_block)
{
	nano::test::system system;
	nano::node_config config = system.default_config ();
	config.websocket_config.enabled = true;
	config.websocket_config.port = system.get_available_port ();
	auto node1 (system.add_node (config));

	std::atomic<bool> ack_ready{ false };
	auto task = ([&ack_ready, config, node1] () {
		fake_websocket_client client (node1->websocket.server->listening_port ());
		client.send_message (R"json({"action": "subscribe", "topic": "new_unconfirmed_block", "ack": true})json");
		client.await_ack ();
		ack_ready = true;
		EXPECT_EQ (1, node1->websocket.server->subscriber_count (nano::websocket::topic::new_unconfirmed_block));
		return client.get_response ();
	});
	auto future = std::async (std::launch::async, task);

	ASSERT_TIMELY (5s, ack_ready);

	nano::state_block_builder builder;
	// Process a new block
	auto send1 = builder
				 .account (nano::dev::genesis_key.pub)
				 .previous (nano::dev::genesis->hash ())
				 .representative (nano::dev::genesis_key.pub)
				 .balance (nano::dev::constants.genesis_amount - 1)
				 .link (nano::dev::genesis_key.pub)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (nano::dev::genesis->hash ()))
				 .build ();

	ASSERT_EQ (nano::block_status::progress, node1->process_local (send1).value ());

	ASSERT_TIMELY_EQ (5s, future.wait_for (0s), std::future_status::ready);

	// Check the response
	boost::optional<std::string> response = future.get ();
	ASSERT_TRUE (response);
	std::stringstream stream;
	stream << response;
	boost::property_tree::ptree event;
	boost::property_tree::read_json (stream, event);
	ASSERT_EQ (event.get<std::string> ("topic"), "new_unconfirmed_block");

	auto message_contents = event.get_child ("message");
	ASSERT_EQ ("state", message_contents.get<std::string> ("type"));
	ASSERT_EQ ("send", message_contents.get<std::string> ("subtype"));
}
