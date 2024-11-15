#include <nano/lib/blocks.hpp>
#include <nano/node/active_elections.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/test_common/chains.hpp>
#include <nano/test_common/network.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

using namespace std::chrono_literals;

TEST (bootstrap_processor, push_one)
{
	nano::test::system system;
	nano::node_config config = system.default_config ();
	config.frontiers_confirmation = nano::frontiers_confirmation_mode::disabled;
	auto node0 (system.add_node (config));
	nano::keypair key1;
	auto node1 = system.make_disconnected_node ();
	auto wallet_id{ nano::random_wallet_id () };
	node1->wallets.create (wallet_id);
	nano::account account;
	ASSERT_EQ (nano::wallets_error::none, node1->wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv, true, account));

	// send 100 raw from genesis to key1
	nano::uint128_t genesis_balance = node1->balance (nano::dev::genesis_key.pub);
	auto send = node1->wallets.send_action (wallet_id, nano::dev::genesis_key.pub, key1.pub, 100);
	ASSERT_NE (nullptr, send);
	ASSERT_TIMELY_EQ (5s, genesis_balance - 100, node1->balance (nano::dev::genesis_key.pub));

	node1->connect (node0->network->endpoint ());
	node1->bootstrap_initiator.bootstrap (node0->network->endpoint ());
	ASSERT_TIMELY_EQ (5s, node0->balance (nano::dev::genesis_key.pub), genesis_balance - 100);
}

TEST (bootstrap_processor, lazy_max_pull_count)
{
	nano::test::system system;
	nano::node_config config = system.default_config ();
	config.frontiers_confirmation = nano::frontiers_confirmation_mode::disabled;
	nano::node_flags node_flags;
	node_flags.set_disable_bootstrap_bulk_push_client (true);
	auto node0 (system.add_node (config, node_flags));
	nano::keypair key1;
	nano::keypair key2;
	// Generating test chain

	nano::state_block_builder builder;

	auto send1 = builder
				 .account (nano::dev::genesis_key.pub)
				 .previous (nano::dev::genesis->hash ())
				 .representative (nano::dev::genesis_key.pub)
				 .balance (nano::dev::constants.genesis_amount - nano::Gxrb_ratio)
				 .link (key1.pub)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*node0->work_generate_blocking (nano::dev::genesis->hash ()))
				 .build ();
	auto receive1 = builder
					.make_block ()
					.account (key1.pub)
					.previous (0)
					.representative (key1.pub)
					.balance (nano::Gxrb_ratio)
					.link (send1->hash ())
					.sign (key1.prv, key1.pub)
					.work (*node0->work_generate_blocking (key1.pub))
					.build ();
	auto send2 = builder
				 .make_block ()
				 .account (key1.pub)
				 .previous (receive1->hash ())
				 .representative (key1.pub)
				 .balance (0)
				 .link (key2.pub)
				 .sign (key1.prv, key1.pub)
				 .work (*node0->work_generate_blocking (receive1->hash ()))
				 .build ();
	auto receive2 = builder
					.make_block ()
					.account (key2.pub)
					.previous (0)
					.representative (key2.pub)
					.balance (nano::Gxrb_ratio)
					.link (send2->hash ())
					.sign (key2.prv, key2.pub)
					.work (*node0->work_generate_blocking (key2.pub))
					.build ();
	auto change1 = builder
				   .make_block ()
				   .account (key2.pub)
				   .previous (receive2->hash ())
				   .representative (key1.pub)
				   .balance (nano::Gxrb_ratio)
				   .link (0)
				   .sign (key2.prv, key2.pub)
				   .work (*node0->work_generate_blocking (receive2->hash ()))
				   .build ();
	auto change2 = builder
				   .make_block ()
				   .account (key2.pub)
				   .previous (change1->hash ())
				   .representative (nano::dev::genesis_key.pub)
				   .balance (nano::Gxrb_ratio)
				   .link (0)
				   .sign (key2.prv, key2.pub)
				   .work (*node0->work_generate_blocking (change1->hash ()))
				   .build ();
	auto change3 = builder
				   .make_block ()
				   .account (key2.pub)
				   .previous (change2->hash ())
				   .representative (key2.pub)
				   .balance (nano::Gxrb_ratio)
				   .link (0)
				   .sign (key2.prv, key2.pub)
				   .work (*node0->work_generate_blocking (change2->hash ()))
				   .build ();
	// Processing test chain
	node0->block_processor.add (send1);
	node0->block_processor.add (receive1);
	node0->block_processor.add (send2);
	node0->block_processor.add (receive2);
	node0->block_processor.add (change1);
	node0->block_processor.add (change2);
	node0->block_processor.add (change3);
	ASSERT_TIMELY (5s, nano::test::exists (*node0, { send1, receive1, send2, receive2, change1, change2, change3 }));

	// Start lazy bootstrap with last block in chain known
	auto node1 = system.make_disconnected_node ();
	nano::test::establish_tcp (system, *node1, node0->network->endpoint ());
	node1->bootstrap_initiator.bootstrap_lazy (change3->hash ());
	// Check processed blocks
	ASSERT_TIMELY (10s, node1->block (change3->hash ()));
}

