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

using namespace std::chrono_literals;

TEST (election, construction)
{
	nano::test::system system (1);
	auto & node = *system.nodes[0];
	auto election = std::make_shared<nano::election> (
	node, nano::dev::genesis, [] (auto const &) {}, [] (auto const &) {}, nano::election_behavior::priority);
}

TEST (election, behavior)
{
	nano::test::system system (1);
	auto chain = nano::test::setup_chain (system, *system.nodes[0], 1, nano::dev::genesis_key, false);
	auto election = nano::test::start_election (system, *system.nodes[0], chain[0]->hash ());
	ASSERT_NE (nullptr, election);
	ASSERT_EQ (nano::election_behavior::manual, election->behavior ());
}

TEST (election, quorum_minimum_flip_success)
{
	nano::test::system system{};

	nano::node_config node_config = system.default_config ();
	node_config.online_weight_minimum = nano::dev::constants.genesis_amount;
	node_config.frontiers_confirmation = nano::frontiers_confirmation_mode::disabled;

	auto & node1 = *system.add_node (node_config);
	auto const latest_hash = nano::dev::genesis->hash ();
	nano::state_block_builder builder{};

	nano::keypair key1{};
	auto send1 = builder.make_block ()
				 .previous (latest_hash)
				 .account (nano::dev::genesis_key.pub)
				 .representative (nano::dev::genesis_key.pub)
				 .balance (node1.quorum ().quorum_delta)
				 .link (key1.pub)
				 .work (*system.work.generate (latest_hash))
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .build ();

	nano::keypair key2{};
	auto send2 = builder.make_block ()
				 .previous (latest_hash)
				 .account (nano::dev::genesis_key.pub)
				 .representative (nano::dev::genesis_key.pub)
				 .balance (node1.quorum ().quorum_delta)
				 .link (key2.pub)
				 .work (*system.work.generate (latest_hash))
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .build ();

	node1.process_active (send1);
	ASSERT_TIMELY (5s, node1.active.election (send1->qualified_root ()) != nullptr)

	node1.process_active (send2);
	std::shared_ptr<nano::election> election{};
	ASSERT_TIMELY (5s, (election = node1.active.election (send2->qualified_root ())) != nullptr)
	ASSERT_TIMELY_EQ (5s, election->blocks ().size (), 2);

	auto vote = nano::test::make_final_vote (nano::dev::genesis_key, { send2->hash () });
	ASSERT_EQ (nano::vote_code::vote, node1.vote (*vote, send2->hash ()));

	ASSERT_TIMELY (5s, node1.active.confirmed (*election));
	auto const winner = election->winner ();
	ASSERT_NE (nullptr, winner);
	ASSERT_EQ (*winner, *send2);
}

