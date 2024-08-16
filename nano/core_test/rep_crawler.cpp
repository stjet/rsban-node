#include <nano/lib/blocks.hpp>
#include <nano/lib/config.hpp>
#include <nano/lib/logging.hpp>
#include <nano/node/active_elections.hpp>
#include <nano/node/repcrawler.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/test_common/chains.hpp>
#include <nano/test_common/network.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

using namespace std::chrono_literals;

// Test that nodes can track nodes that have rep weight for priority broadcasting
TEST (rep_crawler, rep_list)
{
	nano::test::system system;
	auto & node1 = *system.add_node ();
	auto & node2 = *system.add_node ();
	auto wallet_id1 = node1.wallets.first_wallet_id ();
	ASSERT_EQ (0, node2.rep_crawler.representative_count ());
	// Node #1 has a rep
	(void)node1.wallets.insert_adhoc (wallet_id1, nano::dev::genesis_key.prv);
	ASSERT_TIMELY_EQ (5s, node2.rep_crawler.representative_count (), 1);
	auto reps = node2.representative_register.representatives ();
	ASSERT_EQ (1, reps.size ());
	ASSERT_EQ (nano::dev::genesis_key.pub, reps[0].get_account ());
}

TEST (rep_crawler, rep_connection_close)
{
	nano::test::system system;
	auto & node1 = *system.add_node ();
	auto & node2 = *system.add_node ();
	// Add working representative (node 2)
	(void)node2.wallets.insert_adhoc (node2.wallets.first_wallet_id (), nano::dev::genesis_key.prv);
	ASSERT_TIMELY_EQ (10s, node1.rep_crawler.representative_count (), 1);
	system.stop_node (node2);
	// Remove representative with closed channel
	ASSERT_TIMELY_EQ (10s, node1.rep_crawler.representative_count (), 0);
}

// This test checks that if a block is in the recently_confirmed list then the repcrawler will not send a request for it.
// The behaviour of this test previously was the opposite, that the repcrawler eventually send out such a block and deleted the block
// from the recently confirmed list to try to make ammends for sending it, which is bad behaviour.
// In the long term, we should have a better way to check for reps and this test should become redundant
TEST (rep_crawler, recently_confirmed)
{
	nano::test::system system (1);
	auto & node1 (*system.nodes[0]);
	ASSERT_EQ (1, node1.ledger.block_count ());
	auto const block = nano::dev::genesis;
	node1.active.insert_recently_confirmed (block);
	auto & node2 (*system.add_node ());
	auto wallet_id2 = node2.wallets.first_wallet_id ();
	(void)node2.wallets.insert_adhoc (wallet_id2, nano::dev::genesis_key.prv);
	auto channel = node1.network->find_node_id (node2.get_node_id ());
	ASSERT_NE (nullptr, channel);
	node1.rep_crawler.query (channel); // this query should be dropped due to the recently_confirmed entry
	ASSERT_ALWAYS_EQ (0.5s, node1.rep_crawler.representative_count (), 0);
}
