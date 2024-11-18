#include "nano/secure/common.hpp"

#include <nano/lib/blocks.hpp>
#include <nano/lib/config.hpp>
#include <nano/lib/locks.hpp>
#include <nano/node/active_elections.hpp>
#include <nano/node/election.hpp>
#include <nano/node/inactive_node.hpp>
#include <nano/node/local_vote_history.hpp>
#include <nano/node/make_store.hpp>
#include <nano/node/scheduler/component.hpp>
#include <nano/node/scheduler/manual.hpp>
#include <nano/node/scheduler/priority.hpp>
#include <nano/node/transport/tcp_listener.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/test_common/network.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

#include <boost/filesystem.hpp>
#include <boost/make_shared.hpp>
#include <boost/optional.hpp>

#include <future>
#include <thread>

using namespace std::chrono_literals;

TEST (node, null_account)
{
	auto const & null_account = nano::account::null ();
	ASSERT_EQ (null_account, nullptr);
	ASSERT_FALSE (null_account != nullptr);

	nano::account default_account{};
	ASSERT_FALSE (default_account == nullptr);
	ASSERT_NE (default_account, nullptr);
}

TEST (node, stop)
{
	nano::test::system system (1);
	ASSERT_EQ (1, system.nodes[0]->wallets.wallet_count ());
	ASSERT_TRUE (true);
}

TEST (node, block_store_path_failure)
{
	nano::test::system system;
	auto service (boost::make_shared<rsnano::async_runtime> (false));
	auto path (nano::unique_path ());
	nano::work_pool pool{ nano::dev::network_params.network, std::numeric_limits<unsigned>::max () };
	auto node (std::make_shared<nano::node> (*service, system.get_available_port (), path, pool));
	system.register_node (node);
	ASSERT_EQ (0, node->wallets.wallet_count ());
}

TEST (node, send_unkeyed)
{
	nano::test::system system (1);
	auto node = system.nodes[0];
	auto wallet_id = node->wallets.first_wallet_id ();
	nano::keypair key2;
	(void)node->wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);
	node->wallets.set_password (wallet_id, nano::keypair ().prv);
	ASSERT_EQ (nullptr, node->wallets.send_action (wallet_id, nano::dev::genesis_key.pub, key2.pub, node->config->receive_minimum.number ()));
}

TEST (node, merge_peers)
{
	nano::test::system system (1);
	std::array<nano::endpoint, 8> endpoints;
	endpoints.fill (nano::endpoint (boost::asio::ip::address_v6::loopback (), system.get_available_port ()));
	endpoints[0] = nano::endpoint (boost::asio::ip::address_v6::loopback (), system.get_available_port ());
	system.nodes[0]->network->merge_peers (endpoints);
	ASSERT_EQ (0, system.nodes[0]->network->size ());
}

TEST (node, working)
{
	auto path (nano::working_path ());
	ASSERT_FALSE (path.empty ());
}

TEST (node_config, random_rep)
{
	auto path (nano::unique_path ());
	nano::node_config config1 (100);
	auto rep (config1.random_representative ());
	ASSERT_NE (config1.preconfigured_representatives.end (), std::find (config1.preconfigured_representatives.begin (), config1.preconfigured_representatives.end (), rep));
}

TEST (node, expire)
{
	std::weak_ptr<nano::node> node0;
	{
		nano::test::system system (1);
		node0 = system.nodes[0];
		auto wallet_id0 = system.nodes[0]->wallets.first_wallet_id ();
		auto & node1 (*system.nodes[0]);
		auto wallet_id1 = node1.wallets.first_wallet_id ();
		(void)system.nodes[0]->wallets.insert_adhoc (wallet_id0, nano::dev::genesis_key.prv);
	}
	ASSERT_TRUE (node0.expired ());
}

TEST (node, fork_keep)
{
	nano::test::system system (2);
	auto & node1 (*system.nodes[0]);
	auto & node2 (*system.nodes[1]);
	auto wallet_id1 = node1.wallets.first_wallet_id ();
	ASSERT_EQ (1, node1.network->size ());
	nano::keypair key1;
	nano::keypair key2;
	nano::send_block_builder builder;
	// send1 and send2 fork to different accounts
	auto send1 = builder.make_block ()
				 .previous (nano::dev::genesis->hash ())
				 .destination (key1.pub)
				 .balance (nano::dev::constants.genesis_amount - 100)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (nano::dev::genesis->hash ()))
				 .build ();
	auto send2 = builder.make_block ()
				 .previous (nano::dev::genesis->hash ())
				 .destination (key2.pub)
				 .balance (nano::dev::constants.genesis_amount - 100)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (nano::dev::genesis->hash ()))
				 .build ();
	node1.process_active (send1);
	node2.process_active (builder.make_block ().from (*send1).build ());
	ASSERT_TIMELY_EQ (5s, 1, node1.active.size ());
	ASSERT_TIMELY_EQ (5s, 1, node2.active.size ());
	(void)node1.wallets.insert_adhoc (wallet_id1, nano::dev::genesis_key.prv);
	// Fill node with forked blocks
	node1.process_active (send2);
	ASSERT_TIMELY (5s, node1.active.active (*send2));
	node2.process_active (builder.make_block ().from (*send2).build ());
	ASSERT_TIMELY (5s, node2.active.active (*send2));
	auto election1 (node2.active.election (nano::qualified_root (nano::dev::genesis->hash (), nano::dev::genesis->hash ())));
	ASSERT_NE (nullptr, election1);
	ASSERT_EQ (1, election1->votes ().size ());
	ASSERT_TRUE (node1.block_or_pruned_exists (send1->hash ()));
	ASSERT_TRUE (node2.block_or_pruned_exists (send1->hash ()));
	// Wait until the genesis rep makes a vote
	ASSERT_TIMELY (1.5min, election1->votes ().size () != 1);
	auto transaction0 (node1.store.tx_begin_read ());
	auto transaction1 (node2.store.tx_begin_read ());
	// The vote should be in agreement with what we already have.
	auto winner (*node2.active.tally (*election1).begin ());
	ASSERT_EQ (*send1, *winner.second);
	ASSERT_EQ (nano::dev::constants.genesis_amount - 100, winner.first);
	ASSERT_TRUE (node1.ledger.any ().block_exists (*transaction0, send1->hash ()));
	ASSERT_TRUE (node2.ledger.any ().block_exists (*transaction1, send1->hash ()));
}

TEST (node, coherent_observer)
{
	nano::test::system system (1);
	auto & node1 (*system.nodes[0]);
	auto wallet_id = node1.wallets.first_wallet_id ();
	node1.observers->blocks.add ([&node1] (nano::election_status const & status_a, std::vector<nano::vote_with_weight_info> const &, nano::account const &, nano::uint128_t const &, bool, bool) {
		auto transaction (node1.store.tx_begin_read ());
		ASSERT_TRUE (node1.ledger.any ().block_exists (*transaction, status_a.get_winner ()->hash ()));
	});
	(void)node1.wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);
	nano::keypair key;
	node1.wallets.send_action (wallet_id, nano::dev::genesis_key.pub, key.pub, 1);
}

TEST (node, balance_observer)
{
	nano::test::system system (1);
	auto & node1 (*system.nodes[0]);
	auto wallet_id = node1.wallets.first_wallet_id ();
	std::atomic<int> balances (0);
	nano::keypair key;
	node1.observers->account_balance.add ([&key, &balances] (nano::account const & account_a, bool is_pending) {
		if (key.pub == account_a && is_pending)
		{
			balances++;
		}
		else if (nano::dev::genesis_key.pub == account_a && !is_pending)
		{
			balances++;
		}
	});
	(void)node1.wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);
	node1.wallets.send_action (wallet_id, nano::dev::genesis_key.pub, key.pub, 1);
	system.deadline_set (10s);
	auto done (false);
	while (!done)
	{
		auto ec = system.poll ();
		done = balances.load () == 2;
		ASSERT_NO_ERROR (ec);
	}
}

/** This checks that a node can be opened (without being blocked) when a write lock is held elsewhere */
TEST (node, dont_write_lock_node)
{
	auto path = nano::unique_path ();

	std::promise<void> write_lock_held_promise;
	std::promise<void> finished_promise;
	std::thread ([&path, &write_lock_held_promise, &finished_promise] () {
		auto store = nano::make_store (path, nano::dev::constants, false, true);

		// Hold write lock open until main thread is done needing it
		auto transaction (store->tx_begin_write ());
		write_lock_held_promise.set_value ();
		finished_promise.get_future ().wait ();
	})
	.detach ();

	write_lock_held_promise.get_future ().wait ();

	// Check inactive node can finish executing while a write lock is open
	nano::node_flags flags{ nano::inactive_node_flag_defaults () };
	nano::inactive_node node (path, flags);
	finished_promise.set_value ();
}

TEST (node, node_sequence)
{
	nano::test::system system (3);
	ASSERT_EQ (0, system.nodes[0]->node_seq);
	ASSERT_EQ (0, system.nodes[0]->node_seq);
	ASSERT_EQ (1, system.nodes[1]->node_seq);
	ASSERT_EQ (2, system.nodes[2]->node_seq);
}

// Confirm a complex dependency graph starting from the first block
TEST (node, dependency_graph)
{
	nano::test::system system;
	nano::node_config config (system.get_available_port ());
	config.frontiers_confirmation = nano::frontiers_confirmation_mode::disabled;
	auto & node = *system.add_node (config);
	auto wallet_id = node.wallets.first_wallet_id ();

	nano::state_block_builder builder;
	nano::keypair key1, key2, key3;

	// Send to key1
	auto gen_send1 = builder.make_block ()
					 .account (nano::dev::genesis_key.pub)
					 .previous (nano::dev::genesis->hash ())
					 .representative (nano::dev::genesis_key.pub)
					 .link (key1.pub)
					 .balance (nano::dev::constants.genesis_amount - 1)
					 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
					 .work (*system.work.generate (nano::dev::genesis->hash ()))
					 .build ();
	// Receive from genesis
	auto key1_open = builder.make_block ()
					 .account (key1.pub)
					 .previous (0)
					 .representative (key1.pub)
					 .link (gen_send1->hash ())
					 .balance (1)
					 .sign (key1.prv, key1.pub)
					 .work (*system.work.generate (key1.pub))
					 .build ();
	// Send to genesis
	auto key1_send1 = builder.make_block ()
					  .account (key1.pub)
					  .previous (key1_open->hash ())
					  .representative (key1.pub)
					  .link (nano::dev::genesis_key.pub)
					  .balance (0)
					  .sign (key1.prv, key1.pub)
					  .work (*system.work.generate (key1_open->hash ()))
					  .build ();
	// Receive from key1
	auto gen_receive = builder.make_block ()
					   .from (*gen_send1)
					   .previous (gen_send1->hash ())
					   .link (key1_send1->hash ())
					   .balance (nano::dev::constants.genesis_amount)
					   .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
					   .work (*system.work.generate (gen_send1->hash ()))
					   .build ();
	// Send to key2
	auto gen_send2 = builder.make_block ()
					 .from (*gen_receive)
					 .previous (gen_receive->hash ())
					 .link (key2.pub)
					 .balance (gen_receive->balance_field ().value ().number () - 2)
					 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
					 .work (*system.work.generate (gen_receive->hash ()))
					 .build ();
	// Receive from genesis
	auto key2_open = builder.make_block ()
					 .account (key2.pub)
					 .previous (0)
					 .representative (key2.pub)
					 .link (gen_send2->hash ())
					 .balance (2)
					 .sign (key2.prv, key2.pub)
					 .work (*system.work.generate (key2.pub))
					 .build ();
	// Send to key3
	auto key2_send1 = builder.make_block ()
					  .account (key2.pub)
					  .previous (key2_open->hash ())
					  .representative (key2.pub)
					  .link (key3.pub)
					  .balance (1)
					  .sign (key2.prv, key2.pub)
					  .work (*system.work.generate (key2_open->hash ()))
					  .build ();
	// Receive from key2
	auto key3_open = builder.make_block ()
					 .account (key3.pub)
					 .previous (0)
					 .representative (key3.pub)
					 .link (key2_send1->hash ())
					 .balance (1)
					 .sign (key3.prv, key3.pub)
					 .work (*system.work.generate (key3.pub))
					 .build ();
	// Send to key1
	auto key2_send2 = builder.make_block ()
					  .from (*key2_send1)
					  .previous (key2_send1->hash ())
					  .link (key1.pub)
					  .balance (key2_send1->balance_field ().value ().number () - 1)
					  .sign (key2.prv, key2.pub)
					  .work (*system.work.generate (key2_send1->hash ()))
					  .build ();
	// Receive from key2
	auto key1_receive = builder.make_block ()
						.from (*key1_send1)
						.previous (key1_send1->hash ())
						.link (key2_send2->hash ())
						.balance (key1_send1->balance_field ().value ().number () + 1)
						.sign (key1.prv, key1.pub)
						.work (*system.work.generate (key1_send1->hash ()))
						.build ();
	// Send to key3
	auto key1_send2 = builder.make_block ()
					  .from (*key1_receive)
					  .previous (key1_receive->hash ())
					  .link (key3.pub)
					  .balance (key1_receive->balance_field ().value ().number () - 1)
					  .sign (key1.prv, key1.pub)
					  .work (*system.work.generate (key1_receive->hash ()))
					  .build ();
	// Receive from key1
	auto key3_receive = builder.make_block ()
						.from (*key3_open)
						.previous (key3_open->hash ())
						.link (key1_send2->hash ())
						.balance (key3_open->balance_field ().value ().number () + 1)
						.sign (key3.prv, key3.pub)
						.work (*system.work.generate (key3_open->hash ()))
						.build ();
	// Upgrade key3
	auto key3_epoch = builder.make_block ()
					  .from (*key3_receive)
					  .previous (key3_receive->hash ())
					  .link (node.ledger.epoch_link (nano::epoch::epoch_1))
					  .balance (key3_receive->balance_field ().value ())
					  .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
					  .work (*system.work.generate (key3_receive->hash ()))
					  .build ();

	ASSERT_EQ (nano::block_status::progress, node.process (gen_send1));
	ASSERT_EQ (nano::block_status::progress, node.process (key1_open));
	ASSERT_EQ (nano::block_status::progress, node.process (key1_send1));
	ASSERT_EQ (nano::block_status::progress, node.process (gen_receive));
	ASSERT_EQ (nano::block_status::progress, node.process (gen_send2));
	ASSERT_EQ (nano::block_status::progress, node.process (key2_open));
	ASSERT_EQ (nano::block_status::progress, node.process (key2_send1));
	ASSERT_EQ (nano::block_status::progress, node.process (key3_open));
	ASSERT_EQ (nano::block_status::progress, node.process (key2_send2));
	ASSERT_EQ (nano::block_status::progress, node.process (key1_receive));
	ASSERT_EQ (nano::block_status::progress, node.process (key1_send2));
	ASSERT_EQ (nano::block_status::progress, node.process (key3_receive));
	ASSERT_EQ (nano::block_status::progress, node.process (key3_epoch));
	ASSERT_TRUE (node.active.empty ());

	// Hash -> Ancestors
	std::unordered_map<nano::block_hash, std::vector<nano::block_hash>> dependency_graph{
		{ key1_open->hash (), { gen_send1->hash () } },
		{ key1_send1->hash (), { key1_open->hash () } },
		{ gen_receive->hash (), { gen_send1->hash (), key1_open->hash () } },
		{ gen_send2->hash (), { gen_receive->hash () } },
		{ key2_open->hash (), { gen_send2->hash () } },
		{ key2_send1->hash (), { key2_open->hash () } },
		{ key3_open->hash (), { key2_send1->hash () } },
		{ key2_send2->hash (), { key2_send1->hash () } },
		{ key1_receive->hash (), { key1_send1->hash (), key2_send2->hash () } },
		{ key1_send2->hash (), { key1_send1->hash () } },
		{ key3_receive->hash (), { key3_open->hash (), key1_send2->hash () } },
		{ key3_epoch->hash (), { key3_receive->hash () } },
	};
	ASSERT_EQ (node.ledger.block_count () - 2, dependency_graph.size ());

	// Start an election for the first block of the dependency graph, and ensure all blocks are eventually confirmed
	(void)node.wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);
	node.start_election (gen_send1);

	ASSERT_NO_ERROR (system.poll_until_true (15s, [&] {
		// Not many blocks should be active simultaneously
		EXPECT_LT (node.active.size (), 6);

		// Ensure that active blocks have their ancestors confirmed
		auto error = std::any_of (dependency_graph.cbegin (), dependency_graph.cend (), [&] (auto entry) {
			if (node.election_active (entry.first))
			{
				for (auto ancestor : entry.second)
				{
					if (!node.block_confirmed (ancestor))
					{
						return true;
					}
				}
			}
			return false;
		});

		EXPECT_FALSE (error);
		return error || node.ledger.cemented_count () == node.ledger.block_count ();
	}));
	ASSERT_EQ (node.ledger.cemented_count (), node.ledger.block_count ());
	ASSERT_TIMELY (5s, node.active.empty ());
}

