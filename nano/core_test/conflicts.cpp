#include <nano/lib/blocks.hpp>
#include <nano/node/active_elections.hpp>
#include <nano/node/election.hpp>
#include <nano/node/scheduler/component.hpp>
#include <nano/node/scheduler/priority.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/test_common/chains.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

#include <boost/variant/get.hpp>

using namespace std::chrono_literals;

TEST (conflicts, add_two)
{
	nano::test::system system{};
	auto const & node = system.add_node ();
	nano::keypair key1, key2, key3;
	auto gk = nano::dev::genesis_key;

	// create 2 new accounts, that receive 1 raw each, all blocks are force confirmed
	auto [send1, open1] = nano::test::setup_new_account (system, *node, 1, gk, key1, gk.pub, true);
	auto [send2, open2] = nano::test::setup_new_account (system, *node, 1, gk, key2, gk.pub, true);
	ASSERT_EQ (5, node->ledger.cemented_count ());

	// send 1 raw to account key3 from key1
	auto send_a = nano::state_block_builder ()
				  .account (key1.pub)
				  .previous (open1->hash ())
				  .representative (nano::dev::genesis_key.pub)
				  .balance (0)
				  .link (key3.pub)
				  .sign (key1.prv, key1.pub)
				  .work (*system.work.generate (open1->hash ()))
				  .build ();

	// send 1 raw to account key3 from key2
	auto send_b = nano::state_block_builder ()
				  .account (key2.pub)
				  .previous (open2->hash ())
				  .representative (nano::dev::genesis_key.pub)
				  .balance (0)
				  .link (key3.pub)
				  .sign (key2.prv, key2.pub)
				  .work (*system.work.generate (open2->hash ()))
				  .build ();

	// activate elections for the previous two send blocks (to account3) that we did not forcefully confirm
	ASSERT_TRUE (nano::test::process (*node, { send_a, send_b }));
	ASSERT_TRUE (nano::test::start_elections (system, *node, { send_a, send_b }));
	ASSERT_TRUE (node->active.election (send_a->qualified_root ()));
	ASSERT_TRUE (node->active.election (send_b->qualified_root ()));
	ASSERT_TIMELY_EQ (5s, node->active.size (), 2);
}
