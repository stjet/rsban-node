#include <nano/lib/blocks.hpp>
#include <nano/lib/config.hpp>
#include <nano/node/network.hpp>
#include <nano/node/nodeconfig.hpp>
#include <nano/node/scheduler/component.hpp>
#include <nano/node/scheduler/priority.hpp>
#include <nano/node/transport/tcp_listener.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/store/component.hpp>
#include <nano/test_common/network.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

#include <boost/iostreams/stream_buffer.hpp>
#include <boost/range/join.hpp>
#include <boost/thread.hpp>

#include <thread>

using namespace std::chrono_literals;

TEST (network, multi_keepalive)
{
	nano::test::system system (1);
	auto node0 = system.nodes[0];
	ASSERT_EQ (0, node0->network->size ());
	auto node1 (std::make_shared<nano::node> (system.async_rt, system.get_available_port (), nano::unique_path (), system.work));
	ASSERT_FALSE (node1->init_error ());
	node1->start ();
	system.nodes.push_back (node1);
	ASSERT_EQ (0, node1->network->size ());
	ASSERT_EQ (0, node0->network->size ());
	node1->connect (node0->network->endpoint ());
	ASSERT_TIMELY (10s, node0->network->size () == 1 && node0->stats->count (nano::stat::type::message, nano::stat::detail::keepalive) >= 1);
	auto node2 (std::make_shared<nano::node> (system.async_rt, system.get_available_port (), nano::unique_path (), system.work));
	ASSERT_FALSE (node2->init_error ());
	node2->start ();
	system.nodes.push_back (node2);
	node2->connect (node0->network->endpoint ());
	// ASSERT_TIMELY (10s, node1->network->size () == 2 && node0->network->size () == 2 && node2->network->size () == 2 && node0->stats->count (nano::stat::type::message, nano::stat::detail::keepalive) >= 2);
	std::this_thread::sleep_for (10s);
	std::cout << "node0: " << node0->network->size () << ", node1: " << node1->network->size () << ", node2: " << node2->network->size () << std::endl;
}

TEST (network, send_valid_confirm_ack)
{
	nano::node_flags node_flags;
	nano::test::system system (2, node_flags);
	auto & node1 (*system.nodes[0]);
	auto & node2 (*system.nodes[1]);
	auto wallet_id1 = node1.wallets.first_wallet_id ();
	auto wallet_id2 = node2.wallets.first_wallet_id ();
	nano::keypair key2;
	(void)node1.wallets.insert_adhoc (wallet_id1, nano::dev::genesis_key.prv);
	(void)node2.wallets.insert_adhoc (wallet_id2, key2.prv);
	nano::block_hash latest1 (node1.latest (nano::dev::genesis_key.pub));
	nano::block_builder builder;
	auto block2 = builder
				  .send ()
				  .previous (latest1)
				  .destination (key2.pub)
				  .balance (50)
				  .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				  .work (*system.work.generate (latest1))
				  .build ();
	nano::block_hash latest2 (node2.latest (nano::dev::genesis_key.pub));
	node1.process_active (std::make_shared<nano::send_block> (*block2));
	// Keep polling until latest block changes
	ASSERT_TIMELY (10s, node2.latest (nano::dev::genesis_key.pub) != latest2);
	// Make sure the balance has decreased after processing the block.
	ASSERT_EQ (50, node2.balance (nano::dev::genesis_key.pub));
}

TEST (network, send_valid_publish)
{
	nano::node_flags node_flags;
	nano::test::system system (2, node_flags);
	auto & node1 (*system.nodes[0]);
	auto & node2 (*system.nodes[1]);
	auto wallet_id1 = node1.wallets.first_wallet_id ();
	auto wallet_id2 = node2.wallets.first_wallet_id ();
	node1.bootstrap_initiator.stop ();
	node2.bootstrap_initiator.stop ();
	(void)node1.wallets.insert_adhoc (wallet_id1, nano::dev::genesis_key.prv);
	nano::keypair key2;
	(void)node2.wallets.insert_adhoc (wallet_id2, key2.prv);
	nano::block_hash latest1 (node1.latest (nano::dev::genesis_key.pub));
	nano::block_builder builder;
	auto block2 = builder
				  .send ()
				  .previous (latest1)
				  .destination (key2.pub)
				  .balance (50)
				  .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				  .work (*system.work.generate (latest1))
				  .build ();
	auto hash2 (block2->hash ());
	nano::block_hash latest2 (node2.latest (nano::dev::genesis_key.pub));
	node2.process_active (std::make_shared<nano::send_block> (*block2));
	ASSERT_TIMELY (10s, node1.stats->count (nano::stat::type::message, nano::stat::detail::publish, nano::stat::dir::in) != 0);
	ASSERT_NE (hash2, latest2);
	ASSERT_TIMELY (10s, node2.latest (nano::dev::genesis_key.pub) != latest2);
	ASSERT_EQ (50, node2.balance (nano::dev::genesis_key.pub));
}

TEST (receivable_processor, send_with_receive)
{
	nano::node_flags node_flags;
	nano::test::system system (2, node_flags);
	auto & node1 (*system.nodes[0]);
	auto & node2 (*system.nodes[1]);
	auto wallet_id1 = node1.wallets.first_wallet_id ();
	auto wallet_id2 = node2.wallets.first_wallet_id ();
	auto amount (std::numeric_limits<nano::uint128_t>::max ());
	nano::keypair key2;
	(void)node1.wallets.insert_adhoc (wallet_id1, nano::dev::genesis_key.prv);
	nano::block_hash latest1 (node1.latest (nano::dev::genesis_key.pub));
	nano::block_builder builder;
	auto block1 = builder
				  .send ()
				  .previous (latest1)
				  .destination (key2.pub)
				  .balance (amount - node1.config->receive_minimum.number ())
				  .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				  .work (*system.work.generate (latest1))
				  .build ();
	ASSERT_EQ (amount, node1.balance (nano::dev::genesis_key.pub));
	ASSERT_EQ (0, node1.balance (key2.pub));
	ASSERT_EQ (amount, node2.balance (nano::dev::genesis_key.pub));
	ASSERT_EQ (0, node2.balance (key2.pub));
	node1.process_active (block1);
	ASSERT_TIMELY (5s, nano::test::exists (node1, { block1 }));
	node2.process_active (block1);
	ASSERT_TIMELY (5s, nano::test::exists (node2, { block1 }));
	ASSERT_EQ (amount - node1.config->receive_minimum.number (), node1.balance (nano::dev::genesis_key.pub));
	ASSERT_EQ (0, node1.balance (key2.pub));
	ASSERT_EQ (amount - node1.config->receive_minimum.number (), node2.balance (nano::dev::genesis_key.pub));
	ASSERT_EQ (0, node2.balance (key2.pub));
	(void)node2.wallets.insert_adhoc (wallet_id2, key2.prv);
	ASSERT_TIMELY (10s, node1.balance (key2.pub) == node1.config->receive_minimum.number () && node2.balance (key2.pub) == node1.config->receive_minimum.number ());
	ASSERT_EQ (amount - node1.config->receive_minimum.number (), node1.balance (nano::dev::genesis_key.pub));
	ASSERT_EQ (node1.config->receive_minimum.number (), node1.balance (key2.pub));
	ASSERT_EQ (amount - node1.config->receive_minimum.number (), node2.balance (nano::dev::genesis_key.pub));
	ASSERT_EQ (node1.config->receive_minimum.number (), node2.balance (key2.pub));
}

TEST (network, receive_weight_change)
{
	nano::test::system system (2);
	auto node1 = system.nodes[0];
	auto node2 = system.nodes[1];
	auto wallet_id1 = node1->wallets.first_wallet_id ();
	auto wallet_id2 = node2->wallets.first_wallet_id ();
	(void)node1->wallets.insert_adhoc (wallet_id1, nano::dev::genesis_key.prv);
	nano::keypair key2;
	(void)node2->wallets.insert_adhoc (wallet_id2, key2.prv);
	(void)node2->wallets.set_representative (wallet_id2, key2.pub);
	ASSERT_NE (nullptr, node1->wallets.send_action (wallet_id1, nano::dev::genesis_key.pub, key2.pub, system.nodes[0]->config->receive_minimum.number ()));
	ASSERT_TIMELY (10s, std::all_of (system.nodes.begin (), system.nodes.end (), [&] (std::shared_ptr<nano::node> const & node_a) { return node_a->weight (key2.pub) == system.nodes[0]->config->receive_minimum.number (); }));
}

