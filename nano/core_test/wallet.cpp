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
