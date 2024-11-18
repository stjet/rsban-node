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

TEST (node, node_receive_quorum)
{
	nano::test::system system (1);
	auto & node1 = *system.nodes[0];
	auto wallet_id = node1.wallets.first_wallet_id ();
	nano::keypair key;
	nano::block_hash previous (node1.latest (nano::dev::genesis_key.pub));
	(void)node1.wallets.insert_adhoc (wallet_id, key.prv);
	auto send = nano::send_block_builder ()
				.previous (previous)
				.destination (key.pub)
				.balance (nano::dev::constants.genesis_amount - nano::Gxrb_ratio)
				.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				.work (*system.work.generate (previous))
				.build ();
	node1.process_active (send);
	ASSERT_TIMELY (10s, node1.block_or_pruned_exists (send->hash ()));
	ASSERT_TIMELY (10s, node1.active.election (nano::qualified_root (previous, previous)) != nullptr);
	auto election (node1.active.election (nano::qualified_root (previous, previous)));
	ASSERT_NE (nullptr, election);
	ASSERT_FALSE (node1.active.confirmed (*election));
	ASSERT_EQ (1, election->votes ().size ());

	nano::test::system system2;
	system2.add_node ();
	auto node2 = system2.nodes[0];
	auto wallet_id2 = node2->wallets.first_wallet_id ();

	(void)node2->wallets.insert_adhoc (wallet_id2, nano::dev::genesis_key.prv);
	ASSERT_TRUE (node1.balance (key.pub).is_zero ());
	node1.connect (node2->network->endpoint ());
	while (node1.balance (key.pub).is_zero ())
	{
		ASSERT_NO_ERROR (system.poll ());
		ASSERT_NO_ERROR (system2.poll ());
	}
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

TEST (node, fork_open_flip)
{
	nano::test::system system (1);
	auto & node1 = *system.nodes[0];
	auto wallet_id = node1.wallets.first_wallet_id ();

	std::shared_ptr<nano::election> election;
	nano::keypair key1;
	nano::keypair rep1;
	nano::keypair rep2;

	// send 1 raw from genesis to key1 on both node1 and node2
	auto send1 = nano::send_block_builder ()
				 .previous (nano::dev::genesis->hash ())
				 .destination (key1.pub)
				 .balance (nano::dev::constants.genesis_amount - 1)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (nano::dev::genesis->hash ()))
				 .build ();
	node1.process_active (send1);

	// We should be keeping this block
	nano::open_block_builder builder;
	auto open1 = builder.make_block ()
				 .source (send1->hash ())
				 .representative (rep1.pub)
				 .account (key1.pub)
				 .sign (key1.prv, key1.pub)
				 .work (*system.work.generate (key1.pub))
				 .build ();

	// create a fork of block open1, this block will lose the election
	auto open2 = builder.make_block ()
				 .source (send1->hash ())
				 .representative (rep2.pub)
				 .account (key1.pub)
				 .sign (key1.prv, key1.pub)
				 .work (*system.work.generate (key1.pub))
				 .build ();
	ASSERT_FALSE (*open1 == *open2);

	// give block open1 to node1, manually trigger an election for open1 and ensure it is in the ledger
	node1.process_active (open1);
	ASSERT_TIMELY (5s, node1.block (open1->hash ()) != nullptr);
	node1.scheduler.manual.push (open1);
	ASSERT_TIMELY (5s, (election = node1.active.election (open1->qualified_root ())) != nullptr);
	election->transition_active ();

	// create node2, with blocks send1 and open2 pre-initialised in the ledger,
	// so that block open1 cannot possibly get in the ledger before open2 via background sync
	system.initialization_blocks.push_back (send1);
	system.initialization_blocks.push_back (open2);
	auto & node2 = *system.add_node ();
	system.initialization_blocks.clear ();

	// ensure open2 is in node2 ledger (and therefore has sideband) and manually trigger an election for open2
	ASSERT_TIMELY (5s, node2.block (open2->hash ()) != nullptr);
	node2.scheduler.manual.push (open2);
	ASSERT_TIMELY (5s, (election = node2.active.election (open2->qualified_root ())) != nullptr);
	election->transition_active ();

	ASSERT_TIMELY_EQ (5s, 2, node1.active.size ());
	ASSERT_TIMELY_EQ (5s, 2, node2.active.size ());

	// allow node1 to vote and wait for open1 to be confirmed on node1
	(void)node1.wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);
	ASSERT_TIMELY (5s, node1.block_confirmed (open1->hash ()));

	// Notify both nodes of both blocks, both nodes will become aware that a fork exists
	node1.process_active (open2);
	node2.process_active (open1);

	ASSERT_TIMELY_EQ (5s, 2, election->votes ().size ()); // one more than expected due to elections having dummy votes

	// Node2 should eventually settle on open1
	ASSERT_TIMELY (10s, node2.block (open1->hash ()));
	ASSERT_TIMELY (5s, node1.block_confirmed (open1->hash ()));
	auto winner = *node2.active.tally (*election).begin ();
	ASSERT_EQ (*open1, *winner.second);
	ASSERT_EQ (nano::dev::constants.genesis_amount - 1, winner.first);

	// check the correct blocks are in the ledgers
	auto transaction1 (node1.store.tx_begin_read ());
	auto transaction2 (node2.store.tx_begin_read ());
	ASSERT_TRUE (node1.ledger.any ().block_exists (*transaction1, open1->hash ()));
	ASSERT_TRUE (node2.ledger.any ().block_exists (*transaction2, open1->hash ()));
	ASSERT_FALSE (node2.ledger.any ().block_exists (*transaction2, open2->hash ()));
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

// Test that if we create a block that isn't confirmed, the bootstrapping processes sync the missing block.
TEST (node, unconfirmed_send)
{
	nano::test::system system{};

	auto & node1 = *system.add_node ();
	auto wallet_id1 = node1.wallets.first_wallet_id ();
	(void)node1.wallets.insert_adhoc (wallet_id1, nano::dev::genesis_key.prv);

	nano::keypair key2{};
	auto & node2 = *system.add_node ();
	auto wallet_id2 = node2.wallets.first_wallet_id ();
	(void)node2.wallets.insert_adhoc (wallet_id2, key2.prv);

	// firstly, send two units from node1 to node2 and expect that both nodes see the block as confirmed
	// (node1 will start an election for it, vote on it and node2 gets synced up)
	auto send1 = node1.wallets.send_action (wallet_id1, nano::dev::genesis_key.pub, key2.pub, 2 * nano::Mxrb_ratio);
	ASSERT_TIMELY (5s, node1.block_confirmed (send1->hash ()));
	ASSERT_TIMELY (5s, node2.block_confirmed (send1->hash ()));

	// wait until receive1 (auto-receive created by wallet) is cemented
	ASSERT_TIMELY_EQ (5s, node2.get_confirmation_height (*node2.store.tx_begin_read (), key2.pub), 1);
	ASSERT_EQ (node2.balance (key2.pub), 2 * nano::Mxrb_ratio);
	auto recv1 = node2.ledger.find_receive_block_by_send_hash (*node2.store.tx_begin_read (), key2.pub, send1->hash ());

	// create send2 to send from node2 to node1 and save it to node2's ledger without triggering an election (node1 does not hear about it)
	auto send2 = nano::state_block_builder{}
				 .make_block ()
				 .account (key2.pub)
				 .previous (recv1->hash ())
				 .representative (nano::dev::genesis_key.pub)
				 .balance (nano::Mxrb_ratio)
				 .link (nano::dev::genesis_key.pub)
				 .sign (key2.prv, key2.pub)
				 .work (*system.work.generate (recv1->hash ()))
				 .build ();
	ASSERT_EQ (nano::block_status::progress, node2.process (send2));

	auto send3 = node2.wallets.send_action (wallet_id2, key2.pub, nano::dev::genesis_key.pub, nano::Mxrb_ratio);
	ASSERT_TIMELY (5s, node2.block_confirmed (send2->hash ()));
	ASSERT_TIMELY (5s, node1.block_confirmed (send2->hash ()));
	ASSERT_TIMELY (5s, node2.block_confirmed (send3->hash ()));
	ASSERT_TIMELY (5s, node1.block_confirmed (send3->hash ()));
	ASSERT_TIMELY_EQ (5s, node2.ledger.cemented_count (), 7);
	ASSERT_TIMELY_EQ (5s, node1.balance (nano::dev::genesis_key.pub), nano::dev::constants.genesis_amount);
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

TEST (node, block_confirm)
{
	nano::node_flags node_flags;
	nano::test::system system (2, node_flags);
	auto & node1 (*system.nodes[0]);
	auto & node2 (*system.nodes[1]);
	auto wallet_id1 = node1.wallets.first_wallet_id ();
	auto wallet_id2 = node2.wallets.first_wallet_id ();
	nano::keypair key;
	nano::state_block_builder builder;
	auto send1 = builder.make_block ()
				 .account (nano::dev::genesis_key.pub)
				 .previous (nano::dev::genesis->hash ())
				 .representative (nano::dev::genesis_key.pub)
				 .balance (nano::dev::constants.genesis_amount - nano::Gxrb_ratio)
				 .link (key.pub)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*node1.work_generate_blocking (nano::dev::genesis->hash ()))
				 .build ();
	// A copy is necessary to avoid data races during ledger processing, which sets the sideband
	auto send1_copy = builder.make_block ()
					  .from (*send1)
					  .build ();
	auto hash1 = send1->hash ();
	auto hash2 = send1_copy->hash ();
	node1.block_processor.add (send1);
	node2.block_processor.add (send1_copy);
	ASSERT_TIMELY (5s, node1.block_or_pruned_exists (send1->hash ()) && node2.block_or_pruned_exists (send1_copy->hash ()));
	ASSERT_TRUE (node1.block_or_pruned_exists (send1->hash ()));
	ASSERT_TRUE (node2.block_or_pruned_exists (send1_copy->hash ()));
	// Confirm send1 on node2 so it can vote for send2
	node2.start_election (send1_copy);
	std::shared_ptr<nano::election> election;
	ASSERT_TIMELY (5s, election = node2.active.election (send1_copy->qualified_root ()));
	// Make node2 genesis representative so it can vote
	(void)node2.wallets.insert_adhoc (wallet_id2, nano::dev::genesis_key.prv);
	ASSERT_TIMELY_EQ (10s, node1.active.recently_cemented_size (), 1);
}

TEST (node, block_processor_signatures)
{
	nano::test::system system{ 1 };
	auto & node1 = *system.nodes[0];
	(void)node1.wallets.insert_adhoc (node1.wallets.first_wallet_id (), nano::dev::genesis_key.prv);
	nano::block_hash latest = system.nodes[0]->latest (nano::dev::genesis_key.pub);
	nano::state_block_builder builder;
	nano::keypair key1;
	nano::keypair key2;
	nano::keypair key3;
	auto send1 = builder.make_block ()
				 .account (nano::dev::genesis_key.pub)
				 .previous (latest)
				 .representative (nano::dev::genesis_key.pub)
				 .balance (nano::dev::constants.genesis_amount - nano::Gxrb_ratio)
				 .link (key1.pub)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*node1.work_generate_blocking (latest))
				 .build ();
	auto send2 = builder.make_block ()
				 .account (nano::dev::genesis_key.pub)
				 .previous (send1->hash ())
				 .representative (nano::dev::genesis_key.pub)
				 .balance (nano::dev::constants.genesis_amount - 2 * nano::Gxrb_ratio)
				 .link (key2.pub)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*node1.work_generate_blocking (send1->hash ()))
				 .build ();
	auto send3 = builder.make_block ()
				 .account (nano::dev::genesis_key.pub)
				 .previous (send2->hash ())
				 .representative (nano::dev::genesis_key.pub)
				 .balance (nano::dev::constants.genesis_amount - 3 * nano::Gxrb_ratio)
				 .link (key3.pub)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*node1.work_generate_blocking (send2->hash ()))
				 .build ();
	// Invalid signature bit
	auto send4 = builder.make_block ()
				 .account (nano::dev::genesis_key.pub)
				 .previous (send3->hash ())
				 .representative (nano::dev::genesis_key.pub)
				 .balance (nano::dev::constants.genesis_amount - 4 * nano::Gxrb_ratio)
				 .link (key3.pub)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*node1.work_generate_blocking (send3->hash ()))
				 .build ();
	auto sig{ send4->block_signature () };
	sig.bytes[32] ^= 0x1;
	send4->signature_set (sig);
	// Invalid signature bit (force)
	auto send5 = builder.make_block ()
				 .account (nano::dev::genesis_key.pub)
				 .previous (send3->hash ())
				 .representative (nano::dev::genesis_key.pub)
				 .balance (nano::dev::constants.genesis_amount - 5 * nano::Gxrb_ratio)
				 .link (key3.pub)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*node1.work_generate_blocking (send3->hash ()))
				 .build ();
	auto signature = send5->block_signature ();
	signature.bytes[31] ^= 0x1;
	send5->signature_set (signature);
	// Invalid signature to unchecked
	node1.unchecked.put (send5->previous (), nano::unchecked_info{ send5 });
	auto receive1 = builder.make_block ()
					.account (key1.pub)
					.previous (0)
					.representative (nano::dev::genesis_key.pub)
					.balance (nano::Gxrb_ratio)
					.link (send1->hash ())
					.sign (key1.prv, key1.pub)
					.work (*node1.work_generate_blocking (key1.pub))
					.build ();
	auto receive2 = builder.make_block ()
					.account (key2.pub)
					.previous (0)
					.representative (nano::dev::genesis_key.pub)
					.balance (nano::Gxrb_ratio)
					.link (send2->hash ())
					.sign (key2.prv, key2.pub)
					.work (*node1.work_generate_blocking (key2.pub))
					.build ();
	// Invalid private key
	auto receive3 = builder.make_block ()
					.account (key3.pub)
					.previous (0)
					.representative (nano::dev::genesis_key.pub)
					.balance (nano::Gxrb_ratio)
					.link (send3->hash ())
					.sign (key2.prv, key3.pub)
					.work (*node1.work_generate_blocking (key3.pub))
					.build ();
	node1.process_active (send1);
	node1.process_active (send2);
	node1.process_active (send3);
	node1.process_active (send4);
	node1.process_active (receive1);
	node1.process_active (receive2);
	node1.process_active (receive3);
	ASSERT_TIMELY (5s, node1.block (receive2->hash ()) != nullptr); // Implies send1, send2, send3, receive1.
	ASSERT_TIMELY_EQ (5s, node1.unchecked.count (), 0);
	ASSERT_EQ (nullptr, node1.block (receive3->hash ())); // Invalid signer
	ASSERT_EQ (nullptr, node1.block (send4->hash ())); // Invalid signature via process_active
	ASSERT_EQ (nullptr, node1.block (send5->hash ())); // Invalid signature via unchecked
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

// Confirm a complex dependency graph. Uses frontiers confirmation which will fail to
// confirm a frontier optimistically then fallback to pessimistic confirmation.
TEST (node, dependency_graph_frontier)
{
	nano::test::system system;
	nano::node_config config (system.get_available_port ());
	config.frontiers_confirmation = nano::frontiers_confirmation_mode::disabled;
	auto & node1 = *system.add_node (config);
	auto wallet_id1 = node1.wallets.first_wallet_id ();
	config.peering_port = system.get_available_port ();
	config.frontiers_confirmation = nano::frontiers_confirmation_mode::always;
	auto & node2 = *system.add_node (config);
	auto wallet_id2 = node2.wallets.first_wallet_id ();

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
					  .link (node1.ledger.epoch_link (nano::epoch::epoch_1))
					  .balance (key3_receive->balance_field ().value ())
					  .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
					  .work (*system.work.generate (key3_receive->hash ()))
					  .build ();

	for (auto const & node : system.nodes)
	{
		auto transaction (node->store.tx_begin_write ());
		ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, gen_send1));
		ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, key1_open));
		ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, key1_send1));
		ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, gen_receive));
		ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, gen_send2));
		ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, key2_open));
		ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, key2_send1));
		ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, key3_open));
		ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, key2_send2));
		ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, key1_receive));
		ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, key1_send2));
		ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, key3_receive));
		ASSERT_EQ (nano::block_status::progress, node->ledger.process (*transaction, key3_epoch));
	}

	// node1 can vote, but only on the first block
	(void)node1.wallets.insert_adhoc (wallet_id1, nano::dev::genesis_key.prv);

	ASSERT_TIMELY (10s, node2.active.active (gen_send1->qualified_root ()));
	node1.start_election (gen_send1);

	ASSERT_TIMELY_EQ (15s, node1.ledger.cemented_count (), node1.ledger.block_count ());
	ASSERT_TIMELY_EQ (15s, node2.ledger.cemented_count (), node2.ledger.block_count ());
}

TEST (node, DISABLED_pruning_age)
{
	nano::test::system system{};

	nano::node_config node_config{ system.get_available_port () };
	// TODO: remove after allowing pruned voting
	node_config.enable_voting = false;
	// Pruning with max age 0
	node_config.max_pruning_age = std::chrono::seconds{ 0 };

	nano::node_flags node_flags{};
	node_flags.set_enable_pruning (true);

	auto & node1 = *system.add_node (node_config, node_flags);
	nano::keypair key1{};
	nano::send_block_builder builder{};
	auto latest_hash = nano::dev::genesis->hash ();

	auto send1 = builder.make_block ()
				 .previous (latest_hash)
				 .destination (key1.pub)
				 .balance (nano::dev::constants.genesis_amount - nano::Gxrb_ratio)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (latest_hash))
				 .build ();
	node1.process_active (send1);

	latest_hash = send1->hash ();
	auto send2 = builder.make_block ()
				 .previous (latest_hash)
				 .destination (key1.pub)
				 .balance (0)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (latest_hash))
				 .build ();
	node1.process_active (send2);

	// Force-confirm both blocks
	node1.process_confirmed (nano::election_status{ send1 });
	ASSERT_TIMELY (5s, node1.block_confirmed (send1->hash ()));
	node1.process_confirmed (nano::election_status{ send2 });
	ASSERT_TIMELY (5s, node1.block_confirmed (send2->hash ()));

	node1.ledger_pruning (1, true);
	ASSERT_EQ (1, node1.ledger.pruned_count ());
	ASSERT_EQ (3, node1.ledger.block_count ());

	ASSERT_TRUE (nano::test::block_or_pruned_all_exists (node1, { nano::dev::genesis, send1, send2 }));
}

// Test that a node configured with `enable_pruning` will
// prune DEEP-enough confirmed blocks by explicitly saying `node.ledger_pruning` in the unit test
TEST (node, DISABLED_pruning_depth)
{
	nano::test::system system{};

	nano::node_config node_config{ system.get_available_port () };
	// TODO: remove after allowing pruned voting
	node_config.enable_voting = false;

	nano::node_flags node_flags{};
	node_flags.set_enable_pruning (true);

	auto & node1 = *system.add_node (node_config, node_flags);
	nano::keypair key1{};
	nano::send_block_builder builder{};
	auto latest_hash = nano::dev::genesis->hash ();

	auto send1 = builder.make_block ()
				 .previous (latest_hash)
				 .destination (key1.pub)
				 .balance (nano::dev::constants.genesis_amount - nano::Gxrb_ratio)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (latest_hash))
				 .build ();
	node1.process_active (send1);

	latest_hash = send1->hash ();
	auto send2 = builder.make_block ()
				 .previous (latest_hash)
				 .destination (key1.pub)
				 .balance (0)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (latest_hash))
				 .build ();
	node1.process_active (send2);

	// Force-confirm both blocks
	node1.process_confirmed (nano::election_status{ send1 });
	ASSERT_TIMELY (5s, node1.block_confirmed (send1->hash ()));
	node1.process_confirmed (nano::election_status{ send2 });
	ASSERT_TIMELY (5s, node1.block_confirmed (send2->hash ()));

	// Three blocks in total, nothing pruned yet
	ASSERT_EQ (0, node1.ledger.pruned_count ());
	ASSERT_EQ (3, node1.ledger.block_count ());

	// Pruning with default depth (unlimited)
	node1.ledger_pruning (1, true);
	ASSERT_EQ (0, node1.ledger.pruned_count ());
	ASSERT_EQ (3, node1.ledger.block_count ());
}

