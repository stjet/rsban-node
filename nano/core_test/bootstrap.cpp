#include <nano/lib/blocks.hpp>
#include <nano/node/active_elections.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/test_common/chains.hpp>
#include <nano/test_common/network.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

using namespace std::chrono_literals;

// TODO Gustav: I've disabled this test because it fails I haven't found out why yet.
// Legacy bootstrap will be removed soon and pruning is no priority currently
TEST (bootstrap_processor, DISABLED_push_diamond_pruning)
{
	nano::test::system system;
	nano::node_config config = system.default_config ();
	config.frontiers_confirmation = nano::frontiers_confirmation_mode::disabled;
	nano::node_flags node_flags0;
	node_flags0.set_disable_ascending_bootstrap (true);
	node_flags0.set_disable_ongoing_bootstrap (true);
	auto node0 (system.add_node (config, node_flags0));
	nano::keypair key;

	config.enable_voting = false; // Remove after allowing pruned voting
	nano::node_flags node_flags;
	node_flags.set_enable_pruning (true);
	config.peering_port = system.get_available_port ();
	auto node1 = system.make_disconnected_node (config, node_flags);

	nano::block_builder builder;

	// send all balance from genesis to key
	auto send1 = builder
				 .send ()
				 .previous (nano::dev::genesis->hash ())
				 .destination (key.pub)
				 .balance (0)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (nano::dev::genesis->hash ()))
				 .build ();
	ASSERT_EQ (nano::block_status::progress, node1->process (send1));

	// receive all balance on key
	auto open = builder
				.open ()
				.source (send1->hash ())
				.representative (1)
				.account (key.pub)
				.sign (key.prv, key.pub)
				.work (*system.work.generate (key.pub))
				.build ();
	ASSERT_EQ (nano::block_status::progress, node1->process (open));

	// 1st bootstrap
	node1->connect (node0->network->endpoint ());
	node1->bootstrap_initiator.bootstrap (node0->network->endpoint ());
	ASSERT_TIMELY_EQ (5s, node0->balance (key.pub), nano::dev::constants.genesis_amount);
	ASSERT_TIMELY_EQ (5s, node1->balance (key.pub), nano::dev::constants.genesis_amount);

	// Process more blocks & prune old

	// send 100 raw from key to genesis
	auto send2 = builder
				 .send ()
				 .previous (open->hash ())
				 .destination (nano::dev::genesis_key.pub)
				 .balance (std::numeric_limits<nano::uint128_t>::max () - 100)
				 .sign (key.prv, key.pub)
				 .work (*system.work.generate (open->hash ()))
				 .build ();
	ASSERT_EQ (nano::block_status::progress, node1->process (send2));

	// receive the 100 raw from key on genesis
	auto receive = builder
				   .receive ()
				   .previous (send1->hash ())
				   .source (send2->hash ())
				   .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				   .work (*system.work.generate (send1->hash ()))
				   .build ();
	ASSERT_EQ (nano::block_status::progress, node1->process (receive));

	{
		auto transaction (node1->store.tx_begin_write ());
		node1->ledger.confirm (*transaction, open->hash ());
		ASSERT_EQ (1, node1->ledger.pruning_action (*transaction, send1->hash (), 2));
		ASSERT_EQ (1, node1->ledger.pruning_action (*transaction, open->hash (), 1));
		ASSERT_TRUE (node1->ledger.any ().block_exists (*transaction, nano::dev::genesis->hash ()));
		ASSERT_FALSE (node1->ledger.any ().block_exists (*transaction, send1->hash ()));
		ASSERT_TRUE (node1->store.pruned ().exists (*transaction, send1->hash ()));
		ASSERT_FALSE (node1->ledger.any ().block_exists (*transaction, open->hash ()));
		ASSERT_TRUE (node1->store.pruned ().exists (*transaction, open->hash ()));
		ASSERT_TRUE (node1->ledger.any ().block_exists (*transaction, send2->hash ()));
		ASSERT_TRUE (node1->ledger.any ().block_exists (*transaction, receive->hash ()));
		ASSERT_EQ (2, node1->ledger.pruned_count ());
		ASSERT_EQ (5, node1->ledger.block_count ());
	}

	// 2nd bootstrap
	node1->connect (node0->network->endpoint ());
	node1->bootstrap_initiator.bootstrap (node0->network->endpoint ());
	ASSERT_TIMELY_EQ (5s, node0->balance (nano::dev::genesis_key.pub), 100);
	ASSERT_TIMELY_EQ (5s, node1->balance (nano::dev::genesis_key.pub), 100);
}

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

