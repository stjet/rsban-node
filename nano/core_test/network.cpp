#include <nano/lib/blocks.hpp>
#include <nano/lib/config.hpp>
#include <nano/node/network.hpp>
#include <nano/node/nodeconfig.hpp>
#include <nano/node/scheduler/component.hpp>
#include <nano/node/scheduler/priority.hpp>
#include <nano/node/transport/tcp_listener.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/store/component.hpp>
#include <nano/test_common/network.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

#include <boost/iostreams/stream_buffer.hpp>
#include <boost/range/join.hpp>
#include <boost/thread.hpp>

#include <thread>

using namespace std::chrono_literals;

TEST (network, receive_weight_change)
{
	nano::test::system system (2);
	auto node1 = system.nodes[0];
	auto node2 = system.nodes[1];
	auto wallet_id1 = node1->wallets.first_wallet_id ();
	auto wallet_id2 = node2->wallets.first_wallet_id ();
	(void)node1->wallets.insert_adhoc (wallet_id1, nano::dev::genesis_key.prv);
	nano::keypair key2;
	(void)node2->wallets.insert_adhoc (wallet_id2, key2.prv);
	(void)node2->wallets.set_representative (wallet_id2, key2.pub);
	ASSERT_NE (nullptr, node1->wallets.send_action (wallet_id1, nano::dev::genesis_key.pub, key2.pub, system.nodes[0]->config->receive_minimum.number ()));
	ASSERT_TIMELY (10s, std::all_of (system.nodes.begin (), system.nodes.end (), [&] (std::shared_ptr<nano::node> const & node_a) { return node_a->weight (key2.pub) == system.nodes[0]->config->receive_minimum.number (); }));
}

