#include <nano/lib/blocks.hpp>
#include <nano/lib/logging.hpp>
#include <nano/node/active_elections.hpp>
#include <nano/node/election.hpp>
#include <nano/node/make_store.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

using namespace std::chrono_literals;

TEST (ledger_confirm, observers)
{
	auto amount (std::numeric_limits<nano::uint128_t>::max ());
	nano::test::system system;
	nano::node_flags node_flags;
	auto node1 = system.add_node (node_flags);
	nano::keypair key1;
	nano::block_hash latest1 (node1->latest (nano::dev::genesis_key.pub));
	nano::block_builder builder;
	auto send1 = builder
				 .send ()
				 .previous (latest1)
				 .destination (key1.pub)
				 .balance (amount - node1->config->receive_minimum.number ())
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (latest1))
				 .build ();

	auto transaction = node1->store.tx_begin_write ();
	ASSERT_EQ (nano::block_status::progress, node1->ledger.process (*transaction, send1));
	node1->ledger.confirm (*transaction, send1->hash ());
	ASSERT_TRUE (node1->ledger.confirmed ().block_exists (*transaction, send1->hash ()));
	ASSERT_EQ (1, node1->stats->count (nano::stat::type::confirmation_height, nano::stat::detail::blocks_confirmed, nano::stat::dir::in));
	ASSERT_EQ (2, node1->ledger.cemented_count ());
}

TEST (ledger_confirm, election_winner_details_clearing_node_process_confirmed)
{
	// Make sure election_winner_details is also cleared if the block never enters the confirmation height processor from node::process_confirmed
	nano::test::system system (1);
	auto node = system.nodes.front ();

	nano::block_builder builder;
	auto send = builder
				.send ()
				.previous (nano::dev::genesis->hash ())
				.destination (nano::dev::genesis_key.pub)
				.balance (nano::dev::constants.genesis_amount - nano::Gxrb_ratio)
				.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				.work (*system.work.generate (nano::dev::genesis->hash ()))
				.build ();
	// Add to election_winner_details. Use an unrealistic iteration so that it should fall into the else case and do a cleanup
	node->active.add_election_winner_details (send->hash (),
	std::make_shared<nano::election> (
	*node, send,
	[] (std::shared_ptr<nano::block> const &) {},
	[] (nano::account const &) {}, nano::election_behavior::priority));
	nano::election_status election;
	election.set_winner (send);
	node->process_confirmed (election, 1000000);
	ASSERT_EQ (0, node->active.election_winner_details_size ());
}
