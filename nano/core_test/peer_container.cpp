#include <nano/node/transport/tcp.hpp>
#include <nano/test_common/network.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

#include <chrono>
#include <memory>

using namespace std::chrono_literals;

TEST (peer_container, empty_peers)
{
	nano::test::system system (1);
	nano::network & network (*system.nodes[0]->network);
	system.nodes[0]->network->cleanup (std::chrono::system_clock::now ());
	ASSERT_EQ (0, network.size ());
}

// Tests the function network not_a_peer function used by the nano::transport::tcp_channels.insert ()
TEST (peer_container, reserved_ip_is_not_a_peer)
{
	nano::test::system system{ 1 };
	auto not_a_peer = [&node = system.nodes[0]] (nano::endpoint endpoint_a) -> bool {
		return node->network->tcp_channels->not_a_peer (endpoint_a, true);
	};

	// The return value as true means an error because the IP address is for reserved use
	ASSERT_TRUE (not_a_peer (nano::transport::map_endpoint_to_v6 (nano::endpoint (boost::asio::ip::address (boost::asio::ip::address_v4 (0x00000001)), 10000))));
	ASSERT_TRUE (not_a_peer (nano::transport::map_endpoint_to_v6 (nano::endpoint (boost::asio::ip::address (boost::asio::ip::address_v4 (0xc0000201)), 10000))));
	ASSERT_TRUE (not_a_peer (nano::transport::map_endpoint_to_v6 (nano::endpoint (boost::asio::ip::address (boost::asio::ip::address_v4 (0xc6336401)), 10000))));
	ASSERT_TRUE (not_a_peer (nano::transport::map_endpoint_to_v6 (nano::endpoint (boost::asio::ip::address (boost::asio::ip::address_v4 (0xcb007101)), 10000))));
	ASSERT_TRUE (not_a_peer (nano::transport::map_endpoint_to_v6 (nano::endpoint (boost::asio::ip::address (boost::asio::ip::address_v4 (0xe9fc0001)), 10000))));
	ASSERT_TRUE (not_a_peer (nano::transport::map_endpoint_to_v6 (nano::endpoint (boost::asio::ip::address (boost::asio::ip::address_v4 (0xf0000001)), 10000))));
	ASSERT_TRUE (not_a_peer (nano::transport::map_endpoint_to_v6 (nano::endpoint (boost::asio::ip::address (boost::asio::ip::address_v4 (0xffffffff)), 10000))));

	// Test with a valid IP address
	ASSERT_FALSE (not_a_peer (nano::transport::map_endpoint_to_v6 (nano::endpoint (boost::asio::ip::address (boost::asio::ip::address_v4 (0x08080808)), 10000))));
}

// Test the TCP channel cleanup function works properly. It is used to remove peers that are not
// exchanging messages after a while.
TEST (peer_container, tcp_channel_cleanup_works)
{
	// TODO reimplement in Rust
}

TEST (peer_container, list_fanout)
{
	// TODO reimplement in Rust
}

// This test is similar to network.filter_invalid_version_using with the difference that
// this one checks for the channel's connection to get stopped when an incoming message
// is from an outdated node version.
//
// Disabled because there is currently no way to use different network version
TEST (DISABLED_peer_container, depeer_on_outdated_version)
{
	// TODO reimplement in Rust
}
