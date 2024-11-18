#include <nano/lib/blocks.hpp>
#include <nano/node/active_elections.hpp>
#include <nano/node/election.hpp>
#include <nano/node/inactive_node.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

#include <thread>

using namespace std::chrono_literals;

TEST (wallets, exists)
{
	nano::test::system system (1);
	auto & node (*system.nodes[0]);
	nano::keypair key1;
	nano::keypair key2;
	ASSERT_FALSE (node.wallets.exists (key1.pub));
	ASSERT_FALSE (node.wallets.exists (key2.pub));
	(void)node.wallets.insert_adhoc (node.wallets.first_wallet_id (), key1.prv);
	ASSERT_TRUE (node.wallets.exists (key1.pub));
	ASSERT_FALSE (node.wallets.exists (key2.pub));
	(void)node.wallets.insert_adhoc (node.wallets.first_wallet_id (), key2.prv);
	ASSERT_TRUE (node.wallets.exists (key1.pub));
	ASSERT_TRUE (node.wallets.exists (key2.pub));
}

TEST (wallets, search_receivable)
{
	for (auto search_all : { false, true })
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
		if (search_all)
		{
			node.wallets.search_receivable_all ();
		}
		else
		{
			(void)node.wallets.search_receivable (wallet_id);
		}
		std::shared_ptr<nano::election> election;
		ASSERT_TIMELY (5s, election = node.active.election (send->qualified_root ()));

		// Erase the key so the confirmation does not trigger an automatic receive
		ASSERT_EQ (nano::wallets_error::none, node.wallets.remove_account (wallet_id, nano::dev::genesis_key.pub));

		// Now confirm the election
		node.active.force_confirm (*election);

		ASSERT_TIMELY (5s, node.block_confirmed (send->hash ()) && node.active.empty ());

		// Re-insert the key
		(void)node.wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);

		// Pending search should create the receive block
		ASSERT_EQ (2, node.ledger.block_count ());
		if (search_all)
		{
			node.wallets.search_receivable_all ();
		}
		else
		{
			(void)node.wallets.search_receivable (wallet_id);
		}
		ASSERT_TIMELY_EQ (3s, node.balance (nano::dev::genesis_key.pub), nano::dev::constants.genesis_amount);
		auto receive_hash = node.ledger.any ().account_head (*node.store.tx_begin_read (), nano::dev::genesis_key.pub);
		auto receive = node.block (receive_hash);
		ASSERT_NE (nullptr, receive);
		ASSERT_EQ (receive->sideband ().height (), 3);
		ASSERT_EQ (send->hash (), receive->source ());
	}
}
