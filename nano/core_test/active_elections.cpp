#include <nano/lib/blocks.hpp>
#include <nano/lib/jsonconfig.hpp>
#include <nano/lib/numbers.hpp>
#include <nano/node/active_elections.hpp>
#include <nano/node/election.hpp>
#include <nano/node/scheduler/component.hpp>
#include <nano/node/scheduler/manual.hpp>
#include <nano/node/scheduler/priority.hpp>
#include <nano/secure/common.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/test_common/chains.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

#include <thread>

using namespace std::chrono_literals;

namespace nano
{
TEST (active_elections, vote_replays)
{
	nano::test::system system;
	nano::node_config node_config = system.default_config ();
	node_config.enable_voting = false;
	node_config.frontiers_confirmation = nano::frontiers_confirmation_mode::disabled;
	auto & node = *system.add_node (node_config);
	nano::keypair key;
	nano::state_block_builder builder;

	// send Gxrb_ratio raw from genesis to key
	auto send1 = builder.make_block ()
				 .account (nano::dev::genesis_key.pub)
				 .previous (nano::dev::genesis->hash ())
				 .representative (nano::dev::genesis_key.pub)
				 .balance (nano::dev::constants.genesis_amount - nano::Gxrb_ratio)
				 .link (key.pub)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (nano::dev::genesis->hash ()))
				 .build ();
	ASSERT_NE (nullptr, send1);

	// create open block for key receing Gxrb_ratio raw
	auto open1 = builder.make_block ()
				 .account (key.pub)
				 .previous (0)
				 .representative (key.pub)
				 .balance (nano::Gxrb_ratio)
				 .link (send1->hash ())
				 .sign (key.prv, key.pub)
				 .work (*system.work.generate (key.pub))
				 .build ();
	ASSERT_NE (nullptr, open1);

	// wait for elections objects to appear in the AEC
	node.process_active (send1);
	node.process_active (open1);
	ASSERT_TRUE (nano::test::start_elections (system, node, { send1, open1 }));
	ASSERT_EQ (2, node.active.size ());

	// First vote is not a replay and confirms the election, second vote should be a replay since the election has confirmed but not yet removed
	auto vote_send1 = nano::test::make_final_vote (nano::dev::genesis_key, { send1 });
	ASSERT_EQ (nano::vote_code::vote, node.vote (*vote_send1, send1->hash ()));
	ASSERT_EQ (nano::vote_code::replay, node.vote (*vote_send1, send1->hash ()));

	// Wait until the election is removed, at which point the vote is still a replay since it's been recently confirmed
	ASSERT_TIMELY_EQ (5s, node.active.size (), 1);
	ASSERT_EQ (nano::vote_code::replay, node.vote (*vote_send1, send1->hash ()));

	// Open new account
	auto vote_open1 = nano::test::make_final_vote (nano::dev::genesis_key, { open1 });
	ASSERT_EQ (nano::vote_code::vote, node.vote (*vote_open1, open1->hash ()));
	ASSERT_EQ (nano::vote_code::replay, node.vote (*vote_open1, open1->hash ()));
	ASSERT_TIMELY (5s, node.active.empty ());
	ASSERT_EQ (nano::vote_code::replay, node.vote (*vote_open1, open1->hash ()));
	ASSERT_EQ (nano::Gxrb_ratio, node.ledger.weight (key.pub));

	// send 1 raw to key to key
	auto send2 = builder.make_block ()
				 .account (key.pub)
				 .previous (open1->hash ())
				 .representative (key.pub)
				 .balance (nano::Gxrb_ratio - 1)
				 .link (key.pub)
				 .sign (key.prv, key.pub)
				 .work (*system.work.generate (open1->hash ()))
				 .build ();
	ASSERT_NE (nullptr, send2);
	node.process_active (send2);
	ASSERT_TRUE (nano::test::start_elections (system, node, { send2 }));
	ASSERT_EQ (1, node.active.size ());

	// vote2_send2 is a non final vote with little weight, vote1_send2 is the vote that confirms the election
	auto vote1_send2 = nano::test::make_final_vote (nano::dev::genesis_key, { send2 });
	auto vote2_send2 = nano::test::make_vote (key, { send2 }, 0, 0);
	ASSERT_EQ (nano::vote_code::vote, node.vote (*vote2_send2, send2->hash ())); // this vote cannot confirm the election
	ASSERT_EQ (1, node.active.size ());
	ASSERT_EQ (nano::vote_code::replay, node.vote (*vote2_send2, send2->hash ())); // this vote cannot confirm the election
	ASSERT_EQ (1, node.active.size ());
	ASSERT_EQ (nano::vote_code::vote, node.vote (*vote1_send2, send2->hash ())); // this vote confirms the election

	// this should still return replay, either because the election is still in the AEC or because it is recently confirmed
	ASSERT_EQ (nano::vote_code::replay, node.vote (*vote1_send2, send2->hash ()));
	ASSERT_TIMELY (5s, node.active.empty ());
	ASSERT_EQ (nano::vote_code::replay, node.vote (*vote1_send2, send2->hash ()));
	ASSERT_EQ (nano::vote_code::replay, node.vote (*vote2_send2, send2->hash ()));

	// Removing blocks as recently confirmed makes every vote indeterminate
	node.active.clear_recently_confirmed ();
	ASSERT_EQ (nano::vote_code::indeterminate, node.vote (*vote_send1, send1->hash ()));
	ASSERT_EQ (nano::vote_code::indeterminate, node.vote (*vote_open1, open1->hash ()));
	ASSERT_EQ (nano::vote_code::indeterminate, node.vote (*vote1_send2, send2->hash ()));
	ASSERT_EQ (nano::vote_code::indeterminate, node.vote (*vote2_send2, send2->hash ()));
}
}

// Tests that blocks are correctly cleared from the duplicate filter for unconfirmed elections
TEST (active_elections, dropped_cleanup)
{
	nano::test::system system;
	nano::node_flags flags;
	flags.set_disable_request_loop (true);
	auto & node (*system.add_node (flags));
	auto chain = nano::test::setup_chain (system, node, 1, nano::dev::genesis_key, false);
	auto hash = chain[0]->hash ();

	// Add to network filter to ensure proper cleanup after the election is dropped
	std::vector<uint8_t> block_bytes;
	{
		nano::vectorstream stream (block_bytes);
		chain[0]->serialize (stream);
	}
	ASSERT_FALSE (node.network->tcp_channels->publish_filter->apply (block_bytes.data (), block_bytes.size ()));
	ASSERT_TRUE (node.network->tcp_channels->publish_filter->apply (block_bytes.data (), block_bytes.size ()));

	auto election = nano::test::start_election (system, node, hash);
	ASSERT_NE (nullptr, election);

	// Not yet removed
	ASSERT_TRUE (node.network->tcp_channels->publish_filter->apply (block_bytes.data (), block_bytes.size ()));
	ASSERT_TRUE (node.election_active (hash));

	// Now simulate dropping the election
	ASSERT_FALSE (node.active.confirmed (*election));
	node.active.erase (*chain[0]);

	// The filter must have been cleared
	ASSERT_FALSE (node.network->tcp_channels->publish_filter->apply (block_bytes.data (), block_bytes.size ()));

	// An election was recently dropped
	ASSERT_EQ (1, node.stats->count (nano::stat::type::active_elections_dropped, nano::stat::detail::manual));

	// Block cleared from active
	ASSERT_FALSE (node.election_active (hash));

	// Repeat test for a confirmed election
	ASSERT_TRUE (node.network->tcp_channels->publish_filter->apply (block_bytes.data (), block_bytes.size ()));

	election = nano::test::start_election (system, node, hash);
	ASSERT_NE (nullptr, election);
	node.active.force_confirm (*election);
	ASSERT_TIMELY (5s, node.active.confirmed (*election));
	node.active.erase (*chain[0]);

	// The filter should not have been cleared
	ASSERT_TRUE (node.network->tcp_channels->publish_filter->apply (block_bytes.data (), block_bytes.size ()));

	// Not dropped
	ASSERT_EQ (1, node.stats->count (nano::stat::type::active_elections_dropped, nano::stat::detail::manual));

	// Block cleared from active
	ASSERT_FALSE (node.election_active (hash));
}

/*
 * What this test is doing:
 * Create 20 representatives with minimum principal weight each
 * Create a send block on the genesis account (the last send block)
 * Create 20 forks of the last send block using genesis as representative (no votes produced)
 * Check that only 10 blocks remain in the election (due to max 10 forks per election object limit)
 * Create 20 more forks of the last send block using the new reps as representatives and produce votes for them
 *     (9 votes from this batch should survive and replace existing blocks in the election, why not 10?)
 * Then send winning block and it should replace one of the existing blocks
 */
// Disabled by Gustav. It is flaky.
TEST (active_elections, DISABLED_fork_replacement_tally)
{
	// TODO reimplement in Rust
}

TEST (active_elections, confirm_new)
{
	nano::test::system system (1);
	auto & node1 = *system.nodes[0];
	auto send = nano::send_block_builder ()
				.previous (nano::dev::genesis->hash ())
				.destination (nano::public_key ())
				.balance (nano::dev::constants.genesis_amount - 100)
				.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				.work (*system.work.generate (nano::dev::genesis->hash ()))
				.build ();
	node1.process_active (send);
	ASSERT_TIMELY_EQ (5s, 1, node1.active.size ());
	auto & node2 = *system.add_node ();
	// Add key to node2
	auto wallet_id = node2.wallets.first_wallet_id ();
	(void)node2.wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);
	// Let node2 know about the block
	auto send_copy = nano::send_block_builder ().make_block ().from (*send).build ();
	ASSERT_TIMELY (5s, node2.block (send_copy->hash ()));
	// Wait confirmation
	ASSERT_TIMELY (5s, node1.ledger.cemented_count () == 2);
	ASSERT_TIMELY (5s, node2.ledger.cemented_count () == 2);
}

TEST (active_elections, activate_inactive)
{
	nano::test::system system;
	nano::node_flags flags;
	nano::node_config config = system.default_config ();
	config.frontiers_confirmation = nano::frontiers_confirmation_mode::disabled;
	auto & node = *system.add_node (config, flags);

	nano::keypair key;
	nano::state_block_builder builder;
	auto send = builder.make_block ()
				.account (nano::dev::genesis_key.pub)
				.previous (nano::dev::genesis->hash ())
				.representative (nano::dev::genesis_key.pub)
				.link (key.pub)
				.balance (nano::dev::constants.genesis_amount - 1)
				.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				.work (*system.work.generate (nano::dev::genesis->hash ()))
				.build ();
	auto send2 = builder.make_block ()
				 .account (nano::dev::genesis_key.pub)
				 .previous (send->hash ())
				 .representative (nano::dev::genesis_key.pub)
				 .link (nano::keypair ().pub)
				 .balance (nano::dev::constants.genesis_amount - 2)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (send->hash ()))
				 .build ();
	auto open = builder.make_block ()
				.account (key.pub)
				.previous (0)
				.representative (key.pub)
				.link (send->hash ())
				.balance (1)
				.sign (key.prv, key.pub)
				.work (*system.work.generate (key.pub))
				.build ();

	ASSERT_EQ (nano::block_status::progress, node.process (send));
	ASSERT_EQ (nano::block_status::progress, node.process (send2));
	ASSERT_EQ (nano::block_status::progress, node.process (open));

	auto election = nano::test::start_election (system, node, send2->hash ());
	ASSERT_NE (nullptr, election);
	node.active.force_confirm (*election);

	ASSERT_TIMELY (5s, !node.confirming_set.exists (send2->hash ()));
	ASSERT_TIMELY (5s, node.block_confirmed (send2->hash ()));
	ASSERT_TIMELY (5s, node.block_confirmed (send->hash ()));

	// wait so that blocks observer can increase the stats
	std::this_thread::sleep_for (1000ms);

	ASSERT_TIMELY_EQ (5s, 1, node.stats->count (nano::stat::type::confirmation_observer, nano::stat::detail::inactive_conf_height, nano::stat::dir::out));
	ASSERT_TIMELY_EQ (5s, 1, node.stats->count (nano::stat::type::confirmation_observer, nano::stat::detail::active_quorum, nano::stat::dir::out));
	ASSERT_ALWAYS_EQ (50ms, 0, node.stats->count (nano::stat::type::confirmation_observer, nano::stat::detail::active_conf_height, nano::stat::dir::out));

	// The first block was not active so no activation takes place
	ASSERT_FALSE (node.active.active (open->qualified_root ()) || node.block_confirmed_or_being_confirmed (open->hash ()));
}

/*
 * Ensures we limit the number of vote hinted elections in AEC
 */
// disabled because it doesn't run after tokio switch
TEST (DISABLED_active_elections, limit_vote_hinted_elections)
{
	// TODO reimplement in Rust
}
