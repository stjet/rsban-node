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
