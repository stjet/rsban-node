#include <nano/crypto_lib/random_pool.hpp>
#include <nano/lib/blocks.hpp>
#include <nano/lib/thread_runner.hpp>
#include <nano/lib/threading.hpp>
#include <nano/node/active_elections.hpp>
#include <nano/node/election.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/store/lmdb/wallet_value.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

#include <boost/filesystem.hpp>

#include <cstdint>

using namespace std::chrono_literals;
unsigned constexpr nano::wallet_store::version_current;

TEST (wallet, epoch_2_validation)
{
	nano::test::system system (1);
	auto & node (*system.nodes[0]);
	auto wallet_id = node.wallets.first_wallet_id ();

	// Upgrade the genesis account to epoch 2
	ASSERT_NE (nullptr, system.upgrade_genesis_epoch (node, nano::epoch::epoch_1));
	ASSERT_NE (nullptr, system.upgrade_genesis_epoch (node, nano::epoch::epoch_2));

	(void)node.wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv, false);

	// Test send and receive blocks
	// An epoch 2 receive block should be generated with lower difficulty with high probability
	auto tries = 0;
	auto max_tries = 20;
	auto amount = node.config->receive_minimum.number ();
	while (++tries < max_tries)
	{
		auto send = node.wallets.send_action (wallet_id, nano::dev::genesis_key.pub, nano::dev::genesis_key.pub, amount, 1);
		ASSERT_NE (nullptr, send);
		ASSERT_EQ (nano::epoch::epoch_2, send->sideband ().details ().epoch ());
		ASSERT_EQ (nano::epoch::epoch_0, send->sideband ().source_epoch ()); // Not used for send state blocks

		auto receive = node.wallets.receive_action (wallet_id, send->hash (), nano::dev::genesis_key.pub, amount, send->destination (), 1);
		ASSERT_NE (nullptr, receive);
		if (nano::dev::network_params.work.difficulty (*receive) < node.network_params.work.get_base ())
		{
			ASSERT_GE (nano::dev::network_params.work.difficulty (*receive), node.network_params.work.get_epoch_2_receive ());
			ASSERT_EQ (nano::epoch::epoch_2, receive->sideband ().details ().epoch ());
			ASSERT_EQ (nano::epoch::epoch_2, receive->sideband ().source_epoch ());
			break;
		}
	}
	ASSERT_LT (tries, max_tries);

	// Test a change block
	ASSERT_NE (nullptr, node.wallets.change_action (wallet_id, nano::dev::genesis_key.pub, nano::keypair ().pub, 1));
}

// Receiving from an upgraded account uses the lower threshold and upgrades the receiving account
TEST (wallet, epoch_2_receive_propagation)
{
	auto tries = 0;
	auto const max_tries = 20;
	while (++tries < max_tries)
	{
		nano::test::system system;
		nano::node_flags node_flags;
		node_flags.set_disable_request_loop (true);
		auto & node (*system.add_node (node_flags));
		auto wallet_id = node.wallets.first_wallet_id ();

		// Upgrade the genesis account to epoch 1
		auto epoch1 = system.upgrade_genesis_epoch (node, nano::epoch::epoch_1);
		ASSERT_NE (nullptr, epoch1);

		nano::keypair key;
		nano::state_block_builder builder;

		// Send and open the account
		(void)node.wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv, false);
		(void)node.wallets.insert_adhoc (wallet_id, key.prv, false);
		auto amount = node.config->receive_minimum.number ();
		auto send1 = node.wallets.send_action (wallet_id, nano::dev::genesis_key.pub, key.pub, amount, 1);
		ASSERT_NE (nullptr, send1);
		ASSERT_NE (nullptr, node.wallets.receive_action (wallet_id, send1->hash (), nano::dev::genesis_key.pub, amount, send1->destination (), 1));

		// Upgrade the genesis account to epoch 2
		auto epoch2 = system.upgrade_genesis_epoch (node, nano::epoch::epoch_2);
		ASSERT_NE (nullptr, epoch2);

		// Send a block
		auto send2 = node.wallets.send_action (wallet_id, nano::dev::genesis_key.pub, key.pub, amount, 1);
		ASSERT_NE (nullptr, send2);

		auto receive2 = node.wallets.receive_action (wallet_id, send2->hash (), key.pub, amount, send2->destination (), 1);
		ASSERT_NE (nullptr, receive2);
		if (nano::dev::network_params.work.difficulty (*receive2) < node.network_params.work.get_base ())
		{
			ASSERT_GE (nano::dev::network_params.work.difficulty (*receive2), node.network_params.work.get_epoch_2_receive ());
			ASSERT_EQ (nano::epoch::epoch_2, node.ledger.version (*node.store.tx_begin_read (), receive2->hash ()));
			ASSERT_EQ (nano::epoch::epoch_2, receive2->sideband ().source_epoch ());
			break;
		}
	}
	ASSERT_LT (tries, max_tries);
}

// Opening an upgraded account uses the lower threshold
TEST (wallet, epoch_2_receive_unopened)
{
	// Ensure the lower receive work is used when receiving
	auto tries = 0;
	auto const max_tries = 20;
	while (++tries < max_tries)
	{
		nano::test::system system;
		nano::node_flags node_flags;
		node_flags.set_disable_request_loop (true);
		auto & node (*system.add_node (node_flags));
		auto wallet_id = node.wallets.first_wallet_id ();

		// Upgrade the genesis account to epoch 1
		auto epoch1 = system.upgrade_genesis_epoch (node, nano::epoch::epoch_1);
		ASSERT_NE (nullptr, epoch1);

		nano::keypair key;
		nano::state_block_builder builder;

		// Send
		(void)node.wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv, false);
		auto amount = node.config->receive_minimum.number ();
		auto send1 = node.wallets.send_action (wallet_id, nano::dev::genesis_key.pub, key.pub, amount, 1);

		// Upgrade unopened account to epoch_2
		auto epoch2_unopened = builder
							   .account (key.pub)
							   .previous (0)
							   .representative (0)
							   .balance (0)
							   .link (node.network_params.ledger.epochs.link (nano::epoch::epoch_2))
							   .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
							   .work (*system.work.generate (key.pub, node.network_params.work.get_epoch_2 ()))
							   .build ();
		ASSERT_EQ (nano::block_status::progress, node.process (epoch2_unopened));

		(void)node.wallets.insert_adhoc (wallet_id, key.prv, false);

		auto receive1 = node.wallets.receive_action (wallet_id, send1->hash (), key.pub, amount, send1->destination (), 1);
		ASSERT_NE (nullptr, receive1);
		if (nano::dev::network_params.work.difficulty (*receive1) < node.network_params.work.get_base ())
		{
			ASSERT_GE (nano::dev::network_params.work.difficulty (*receive1), node.network_params.work.get_epoch_2_receive ());
			ASSERT_EQ (nano::epoch::epoch_2, node.ledger.version (*node.store.tx_begin_read (), receive1->hash ()));
			ASSERT_EQ (nano::epoch::epoch_1, receive1->sideband ().source_epoch ());
			break;
		}
	}
	ASSERT_LT (tries, max_tries);
}

/**
 * This test checks that wallets::foreach_representative can be used recursively
 */
TEST (wallet, foreach_representative_deadlock)
{
	nano::test::system system (1);
	auto & node (*system.nodes[0]);
	auto wallet_id = node.wallets.first_wallet_id ();
	(void)node.wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);
	node.wallets.compute_reps ();
	ASSERT_EQ (1, node.wallets.voting_reps_count ());

	bool set = false;
	node.wallets.foreach_representative ([&node, &set, &system] (nano::public_key const & pub, nano::raw_key const & prv) {
		node.wallets.foreach_representative ([&node, &set, &system] (nano::public_key const & pub, nano::raw_key const & prv) {
			ASSERT_TIMELY (5s, node.wallets.mutex.try_lock ().has_value ());
			set = true;
		});
	});
	ASSERT_TRUE (set);
}

TEST (wallet, search_receivable)
{
	nano::test::system system;
	nano::node_config config = system.default_config ();
	config.enable_voting = false;
	config.frontiers_confirmation = nano::frontiers_confirmation_mode::disabled;
	nano::node_flags flags;
	flags.set_disable_search_pending (true);
	auto & node (*system.add_node (config, flags));
	auto wallet_id = node.wallets.first_wallet_id ();

	(void)node.wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);
	nano::block_builder builder;
	auto send = builder.state ()
				.account (nano::dev::genesis_key.pub)
				.previous (nano::dev::genesis->hash ())
				.representative (nano::dev::genesis_key.pub)
				.balance (nano::dev::constants.genesis_amount - node.config->receive_minimum.number ())
				.link (nano::dev::genesis_key.pub)
				.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				.work (*system.work.generate (nano::dev::genesis->hash ()))
				.build ();
	ASSERT_EQ (nano::block_status::progress, node.process (send));

	// Pending search should start an election
	ASSERT_TRUE (node.active.empty ());
	ASSERT_EQ (nano::wallets_error::none, node.wallets.search_receivable (wallet_id));
	std::shared_ptr<nano::election> election;
	ASSERT_TIMELY (5s, election = node.active.election (send->qualified_root ()));

	// Erase the key so the confirmation does not trigger an automatic receive
	auto genesis_account = nano::dev::genesis_key.pub;
	ASSERT_EQ (nano::wallets_error::none, node.wallets.remove_account (wallet_id, genesis_account));

	// Now confirm the election
	node.active.force_confirm (*election);

	ASSERT_TIMELY (5s, node.block_confirmed (send->hash ()) && node.active.empty ());

	// Re-insert the key
	(void)node.wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);

	// Pending search should create the receive block
	ASSERT_EQ (2, node.ledger.block_count ());
	ASSERT_EQ (nano::wallets_error::none, node.wallets.search_receivable (wallet_id));
	ASSERT_TIMELY_EQ (3s, node.balance (nano::dev::genesis_key.pub), nano::dev::constants.genesis_amount);
	auto receive_hash = node.ledger.any ().account_head (*node.store.tx_begin_read (), nano::dev::genesis_key.pub);
	auto receive = node.block (receive_hash);
	ASSERT_NE (nullptr, receive);
	ASSERT_EQ (receive->sideband ().height (), 3);
	ASSERT_EQ (send->hash (), receive->source ());
}

TEST (wallet, receive_pruned)
{
	nano::test::system system;
	nano::node_flags node_flags;
	node_flags.set_disable_request_loop (true);
	auto & node1 = *system.add_node (node_flags);
	node_flags.set_enable_pruning (true);
	nano::node_config config = system.default_config ();
	config.enable_voting = false; // Remove after allowing pruned voting
	auto & node2 = *system.add_node (config, node_flags);

	auto wallet_id1 = node1.wallets.first_wallet_id ();
	auto wallet_id2 = node2.wallets.first_wallet_id ();

	nano::keypair key;
	nano::state_block_builder builder;

	// Send
	(void)node1.wallets.insert_adhoc (wallet_id1, nano::dev::genesis_key.prv, false);
	auto amount = node2.config->receive_minimum.number ();
	auto send1 = node1.wallets.send_action (wallet_id1, nano::dev::genesis_key.pub, key.pub, amount, 1);
	auto send2 = node1.wallets.send_action (wallet_id1, nano::dev::genesis_key.pub, key.pub, 1, 1);

	// Pruning
	ASSERT_TIMELY_EQ (5s, node2.ledger.cemented_count (), 3);
	{
		auto transaction = node2.store.tx_begin_write ();
		ASSERT_EQ (1, node2.ledger.pruning_action (*transaction, send1->hash (), 2));
	}
	ASSERT_EQ (1, node2.ledger.pruned_count ());
	ASSERT_TRUE (node2.block_or_pruned_exists (send1->hash ()));
	ASSERT_FALSE (node2.ledger.any ().block_exists (*node2.store.tx_begin_read (), send1->hash ()));

	(void)node2.wallets.insert_adhoc (wallet_id2, key.prv, false);

	auto open1 = node2.wallets.receive_action (wallet_id2, send1->hash (), key.pub, amount, send1->destination (), 1);
	ASSERT_NE (nullptr, open1);
	ASSERT_EQ (amount, node2.ledger.any ().block_balance (*node2.store.tx_begin_read (), open1->hash ()));
	ASSERT_TIMELY_EQ (5s, node2.ledger.cemented_count (), 4);
}
