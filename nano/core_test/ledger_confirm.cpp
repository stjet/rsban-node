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

TEST (ledger_confirm, send_receive_between_2_accounts)
{
	nano::test::system system;
	nano::node_flags node_flags;
	nano::node_config node_config = system.default_config ();
	node_config.frontiers_confirmation = nano::frontiers_confirmation_mode::disabled;
	auto node = system.add_node (node_config, node_flags);
	nano::keypair key1;
	nano::block_hash latest (node->latest (nano::dev::genesis_key.pub));

	nano::block_builder builder;
	auto send1 = builder
				 .send ()
				 .previous (latest)
				 .destination (key1.pub)
				 .balance (node->quorum ().quorum_delta.number () + 2)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (latest))
				 .build ();
	auto open1 = builder
				 .open ()
				 .source (send1->hash ())
				 .representative (nano::dev::genesis_key.pub)
				 .account (key1.pub)
				 .sign (key1.prv, key1.pub)
				 .work (*system.work.generate (key1.pub))
				 .build ();
	auto send2 = builder
				 .send ()
				 .previous (open1->hash ())
				 .destination (nano::dev::genesis_key.pub)
				 .balance (1000)
				 .sign (key1.prv, key1.pub)
				 .work (*system.work.generate (open1->hash ()))
				 .build ();
	auto send3 = builder
				 .send ()
				 .previous (send2->hash ())
				 .destination (nano::dev::genesis_key.pub)
				 .balance (900)
				 .sign (key1.prv, key1.pub)
				 .work (*system.work.generate (send2->hash ()))
				 .build ();
	auto send4 = builder
				 .send ()
				 .previous (send3->hash ())
				 .destination (nano::dev::genesis_key.pub)
				 .balance (500)
				 .sign (key1.prv, key1.pub)
				 .work (*system.work.generate (send3->hash ()))
				 .build ();
	auto receive1 = builder
					.receive ()
					.previous (send1->hash ())
					.source (send2->hash ())
					.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
					.work (*system.work.generate (send1->hash ()))
					.build ();
	auto receive2 = builder
					.receive ()
					.previous (receive1->hash ())
					.source (send3->hash ())
					.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
					.work (*system.work.generate (receive1->hash ()))
					.build ();
	auto receive3 = builder
					.receive ()
					.previous (receive2->hash ())
					.source (send4->hash ())
					.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
					.work (*system.work.generate (receive2->hash ()))
					.build ();
	auto send5 = builder
				 .send ()
				 .previous (receive3->hash ())
				 .destination (key1.pub)
				 .balance (node->quorum ().quorum_delta.number () + 1)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (receive3->hash ()))
				 .build ();
	auto receive4 = builder
					.receive ()
					.previous (send4->hash ())
					.source (send5->hash ())
					.sign (key1.prv, key1.pub)
					.work (*system.work.generate (send4->hash ()))
					.build ();
	nano::keypair key2;
	auto send6 = builder
				 .send ()
				 .previous (send5->hash ())
				 .destination (key2.pub)
				 .balance (node->quorum ().quorum_delta.number () + 1)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (send5->hash ()))
				 .build ();
	// Unpocketed send

	auto transaction = node->store.tx_begin_write ();
	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, send1));
	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, open1));

	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, send2));
	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, receive1));

	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, send3));
	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, send4));

	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, receive2));
	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, receive3));

	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, send5));
	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, send6));

	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, receive4));
	auto confirmed = node->ledger.confirm (*transaction, receive4->hash ());
	ASSERT_EQ (10, confirmed.size ());
	ASSERT_EQ (10, node->stats->count (nano::stat::type::confirmation_height, nano::stat::detail::blocks_confirmed, nano::stat::dir::in));
	ASSERT_EQ (11, node->ledger.cemented_count ());

	ASSERT_TRUE (node->ledger.confirmed ().block_exists (*transaction, receive4->hash ()));
	ASSERT_EQ (7, node->ledger.any ().account_get (*transaction, nano::dev::genesis_key.pub).value ().block_count ());
	ASSERT_EQ (6, node->store.confirmation_height ().get (*transaction, nano::dev::genesis_key.pub).value ().height ());
	ASSERT_EQ (send5->hash (), node->store.confirmation_height ().get (*transaction, nano::dev::genesis_key.pub).value ().frontier ());

	ASSERT_EQ (5, node->ledger.any ().account_get (*transaction, key1.pub).value ().block_count ());
	ASSERT_EQ (5, node->store.confirmation_height ().get (*transaction, key1.pub).value ().height ());
	ASSERT_EQ (receive4->hash (), node->store.confirmation_height ().get (*transaction, key1.pub).value ().frontier ());
}

TEST (ledger_confirm, send_receive_self)
{
	nano::test::system system;
	nano::node_flags node_flags;
	nano::node_config node_config = system.default_config ();
	node_config.frontiers_confirmation = nano::frontiers_confirmation_mode::disabled;
	auto node = system.add_node (node_config, node_flags);
	nano::block_hash latest (node->latest (nano::dev::genesis_key.pub));

	nano::block_builder builder;
	auto send1 = builder
				 .send ()
				 .previous (latest)
				 .destination (nano::dev::genesis_key.pub)
				 .balance (nano::dev::constants.genesis_amount - 2)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (latest))
				 .build ();
	auto receive1 = builder
					.receive ()
					.previous (send1->hash ())
					.source (send1->hash ())
					.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
					.work (*system.work.generate (send1->hash ()))
					.build ();
	auto send2 = builder
				 .send ()
				 .previous (receive1->hash ())
				 .destination (nano::dev::genesis_key.pub)
				 .balance (nano::dev::constants.genesis_amount - 2)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (receive1->hash ()))
				 .build ();
	auto send3 = builder
				 .send ()
				 .previous (send2->hash ())
				 .destination (nano::dev::genesis_key.pub)
				 .balance (nano::dev::constants.genesis_amount - 3)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (send2->hash ()))
				 .build ();
	auto receive2 = builder
					.receive ()
					.previous (send3->hash ())
					.source (send2->hash ())
					.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
					.work (*system.work.generate (send3->hash ()))
					.build ();
	auto receive3 = builder
					.receive ()
					.previous (receive2->hash ())
					.source (send3->hash ())
					.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
					.work (*system.work.generate (receive2->hash ()))
					.build ();

	// Send to another account to prevent automatic receiving on the genesis account
	nano::keypair key1;
	auto send4 = builder
				 .send ()
				 .previous (receive3->hash ())
				 .destination (key1.pub)
				 .balance (node->quorum ().quorum_delta)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (receive3->hash ()))
				 .build ();

	auto transaction = node->store.tx_begin_write ();
	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, send1));
	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, receive1));
	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, send2));
	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, send3));

	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, receive2));
	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, receive3));
	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, send4));

	auto confirmed = node->ledger.confirm (*transaction, receive3->hash ());
	ASSERT_EQ (6, confirmed.size ());
	ASSERT_EQ (6, node->stats->count (nano::stat::type::confirmation_height, nano::stat::detail::blocks_confirmed, nano::stat::dir::in));

	ASSERT_TRUE (node->ledger.confirmed ().block_exists (*transaction, receive3->hash ()));
	ASSERT_EQ (8, node->ledger.any ().account_get (*transaction, nano::dev::genesis_key.pub).value ().block_count ());
	ASSERT_EQ (7, node->store.confirmation_height ().get (*transaction, nano::dev::genesis_key.pub).value ().height ());
	ASSERT_EQ (receive3->hash (), node->store.confirmation_height ().get (*transaction, nano::dev::genesis_key.pub).value ().frontier ());
	ASSERT_EQ (7, node->ledger.cemented_count ());
}

TEST (ledger_confirm, all_block_types)
{
	nano::test::system system;
	nano::node_flags node_flags;
	nano::node_config node_config = system.default_config ();
	node_config.frontiers_confirmation = nano::frontiers_confirmation_mode::disabled;
	auto node = system.add_node (node_config, node_flags);
	nano::block_hash latest (node->latest (nano::dev::genesis_key.pub));
	nano::keypair key1;
	nano::keypair key2;
	auto & store = node->store;
	nano::block_builder builder;
	auto send = builder
				.send ()
				.previous (latest)
				.destination (key1.pub)
				.balance (nano::dev::constants.genesis_amount - nano::Gxrb_ratio)
				.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				.work (*system.work.generate (latest))
				.build ();
	auto send1 = builder
				 .send ()
				 .previous (send->hash ())
				 .destination (key2.pub)
				 .balance (nano::dev::constants.genesis_amount - nano::Gxrb_ratio * 2)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (send->hash ()))
				 .build ();

	auto open = builder
				.open ()
				.source (send->hash ())
				.representative (nano::dev::genesis_key.pub)
				.account (key1.pub)
				.sign (key1.prv, key1.pub)
				.work (*system.work.generate (key1.pub))
				.build ();
	auto state_open = builder
					  .state ()
					  .account (key2.pub)
					  .previous (0)
					  .representative (0)
					  .balance (nano::Gxrb_ratio)
					  .link (send1->hash ())
					  .sign (key2.prv, key2.pub)
					  .work (*system.work.generate (key2.pub))
					  .build ();

	auto send2 = builder
				 .send ()
				 .previous (open->hash ())
				 .destination (key2.pub)
				 .balance (0)
				 .sign (key1.prv, key1.pub)
				 .work (*system.work.generate (open->hash ()))
				 .build ();
	auto state_receive = builder
						 .state ()
						 .account (key2.pub)
						 .previous (state_open->hash ())
						 .representative (0)
						 .balance (nano::Gxrb_ratio * 2)
						 .link (send2->hash ())
						 .sign (key2.prv, key2.pub)
						 .work (*system.work.generate (state_open->hash ()))
						 .build ();

	auto state_send = builder
					  .state ()
					  .account (key2.pub)
					  .previous (state_receive->hash ())
					  .representative (0)
					  .balance (nano::Gxrb_ratio)
					  .link (key1.pub)
					  .sign (key2.prv, key2.pub)
					  .work (*system.work.generate (state_receive->hash ()))
					  .build ();
	auto receive = builder
				   .receive ()
				   .previous (send2->hash ())
				   .source (state_send->hash ())
				   .sign (key1.prv, key1.pub)
				   .work (*system.work.generate (send2->hash ()))
				   .build ();

	auto change = builder
				  .change ()
				  .previous (receive->hash ())
				  .representative (key2.pub)
				  .sign (key1.prv, key1.pub)
				  .work (*system.work.generate (receive->hash ()))
				  .build ();

	auto state_change = builder
						.state ()
						.account (key2.pub)
						.previous (state_send->hash ())
						.representative (nano::dev::genesis_key.pub)
						.balance (nano::Gxrb_ratio)
						.link (0)
						.sign (key2.prv, key2.pub)
						.work (*system.work.generate (state_send->hash ()))
						.build ();

	auto epoch = builder
				 .state ()
				 .account (key2.pub)
				 .previous (state_change->hash ())
				 .representative (nano::dev::genesis_key.pub)
				 .balance (nano::Gxrb_ratio)
				 .link (node->ledger.epoch_link (nano::epoch::epoch_1))
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (state_change->hash ()))
				 .build ();

	auto epoch1 = builder
				  .state ()
				  .account (key1.pub)
				  .previous (change->hash ())
				  .representative (key2.pub)
				  .balance (nano::Gxrb_ratio)
				  .link (node->ledger.epoch_link (nano::epoch::epoch_1))
				  .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				  .work (*system.work.generate (change->hash ()))
				  .build ();
	auto state_send1 = builder
					   .state ()
					   .account (key1.pub)
					   .previous (epoch1->hash ())
					   .representative (0)
					   .balance (nano::Gxrb_ratio - 1)
					   .link (key2.pub)
					   .sign (key1.prv, key1.pub)
					   .work (*system.work.generate (epoch1->hash ()))
					   .build ();
	auto state_receive2 = builder
						  .state ()
						  .account (key2.pub)
						  .previous (epoch->hash ())
						  .representative (0)
						  .balance (nano::Gxrb_ratio + 1)
						  .link (state_send1->hash ())
						  .sign (key2.prv, key2.pub)
						  .work (*system.work.generate (epoch->hash ()))
						  .build ();

	auto state_send2 = builder
					   .state ()
					   .account (key2.pub)
					   .previous (state_receive2->hash ())
					   .representative (0)
					   .balance (nano::Gxrb_ratio)
					   .link (key1.pub)
					   .sign (key2.prv, key2.pub)
					   .work (*system.work.generate (state_receive2->hash ()))
					   .build ();
	auto state_send3 = builder
					   .state ()
					   .account (key2.pub)
					   .previous (state_send2->hash ())
					   .representative (0)
					   .balance (nano::Gxrb_ratio - 1)
					   .link (key1.pub)
					   .sign (key2.prv, key2.pub)
					   .work (*system.work.generate (state_send2->hash ()))
					   .build ();

	auto state_send4 = builder
					   .state ()
					   .account (key1.pub)
					   .previous (state_send1->hash ())
					   .representative (0)
					   .balance (nano::Gxrb_ratio - 2)
					   .link (nano::dev::genesis_key.pub)
					   .sign (key1.prv, key1.pub)
					   .work (*system.work.generate (state_send1->hash ()))
					   .build ();
	auto state_receive3 = builder
						  .state ()
						  .account (nano::dev::genesis_key.pub)
						  .previous (send1->hash ())
						  .representative (nano::dev::genesis_key.pub)
						  .balance (nano::dev::constants.genesis_amount - nano::Gxrb_ratio * 2 + 1)
						  .link (state_send4->hash ())
						  .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
						  .work (*system.work.generate (send1->hash ()))
						  .build ();

	auto transaction (store.tx_begin_write ());
	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, send));
	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, send1));
	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, open));
	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, state_open));

	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, send2));
	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, state_receive));

	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, state_send));
	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, receive));
	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, change));
	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, state_change));

	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, epoch));
	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, epoch1));

	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, state_send1));
	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, state_receive2));

	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, state_send2));
	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, state_send3));

	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, state_send4));
	ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, state_receive3));

	auto confirmed = node->ledger.confirm (*transaction, state_send2->hash ());
	ASSERT_EQ (15, confirmed.size ());
	ASSERT_EQ (15, node->stats->count (nano::stat::type::confirmation_height, nano::stat::detail::blocks_confirmed, nano::stat::dir::in));
	ASSERT_EQ (16, node->ledger.cemented_count ());

	ASSERT_TRUE (node->ledger.confirmed ().block_exists (*transaction, state_send2->hash ()));
	nano::confirmation_height_info confirmation_height_info;
	ASSERT_LE (4, node->ledger.any ().account_get (*transaction, nano::dev::genesis_key.pub).value ().block_count ());
	ASSERT_EQ (3, node->store.confirmation_height ().get (*transaction, nano::dev::genesis_key.pub).value ().height ());
	ASSERT_EQ (send1->hash (), node->store.confirmation_height ().get (*transaction, nano::dev::genesis_key.pub).value ().frontier ());

	ASSERT_LE (7, node->ledger.any ().account_get (*transaction, key1.pub).value ().block_count ());
	ASSERT_EQ (6, node->store.confirmation_height ().get (*transaction, key1.pub).value ().height ());
	ASSERT_EQ (state_send1->hash (), node->store.confirmation_height ().get (*transaction, key1.pub).value ().frontier ());

	ASSERT_EQ (8, node->ledger.any ().account_get (*transaction, key2.pub).value ().block_count ());
	ASSERT_EQ (7, node->store.confirmation_height ().get (*transaction, key2.pub).value ().height ());
	ASSERT_EQ (state_send2->hash (), node->store.confirmation_height ().get (*transaction, key2.pub).value ().frontier ());
}

// This test ensures a block that's cemented cannot be rolled back by the node
// A block is inserted and confirmed then later a different block is force inserted with a rollback attempt
TEST (ledger_confirm, conflict_rollback_cemented)
{
	nano::state_block_builder builder;
	auto const genesis_hash = nano::dev::genesis->hash ();

	nano::test::system system;
	nano::node_flags node_flags;
	auto node1 = system.add_node (node_flags);

	nano::keypair key1;
	// create one side of a forked transaction on node1
	auto fork1a = builder.make_block ()
				  .previous (genesis_hash)
				  .account (nano::dev::genesis_key.pub)
				  .representative (nano::dev::genesis_key.pub)
				  .link (key1.pub)
				  .balance (nano::dev::constants.genesis_amount - 100)
				  .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				  .work (*system.work.generate (genesis_hash))
				  .build ();
	{
		auto transaction = node1->store.tx_begin_write ();
		ASSERT_EQ (nano::block_status::progress, node1->ledger.process (*transaction, fork1a));
		node1->ledger.confirm (*transaction, fork1a->hash ());
	}
	ASSERT_TRUE (nano::test::confirmed (*node1, { fork1a }));

	// create the other side of the fork on node2
	nano::keypair key2;
	auto fork1b = builder.make_block ()
				  .previous (genesis_hash)
				  .account (nano::dev::genesis_key.pub)
				  .representative (nano::dev::genesis_key.pub)
				  .link (key2.pub) // Different destination same 'previous'
				  .balance (nano::dev::constants.genesis_amount - 100)
				  .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				  .work (*system.work.generate (genesis_hash))
				  .build ();

	node1->block_processor.force (fork1b);
	// node2 already has send2 forced confirmed whilst node1 should have confirmed send1 and therefore we have a cemented fork on node2
	// and node2 should print an error message on the log that it cannot rollback send2 because it is already cemented
	[[maybe_unused]] size_t count = 0;
	ASSERT_TIMELY_EQ (5s, 1, (count = node1->stats->count (nano::stat::type::ledger, nano::stat::detail::rollback_failed)));
	ASSERT_TRUE (nano::test::confirmed (*node1, { fork1a->hash () })); // fork1a should still remain after the rollback failed event
}

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
