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

TEST (wallets, open_existing)
{
	nano::test::system system (1);
	auto id (nano::random_wallet_id ());
	{
		nano::wallets wallets (*system.nodes[0]);
		ASSERT_EQ (1, wallets.wallet_count ());
		wallets.create (id);
		ASSERT_TRUE (wallets.wallet_exists (id));
		nano::raw_key password;
		password.clear ();
		system.deadline_set (10s);
		while (password == 0)
		{
			ASSERT_NO_ERROR (system.poll ());
			wallets.password (id, password);
		}
	}
	{
		nano::wallets wallets (*system.nodes[0]);
		ASSERT_EQ (2, wallets.wallet_count ());
		ASSERT_TRUE (wallets.wallet_exists (id));
		// give it some time so that the receivable blocks search can run
		std::this_thread::sleep_for (1000ms);
	}
}

TEST (wallets, remove)
{
	nano::test::system system (1);
	nano::wallet_id one (1);
	{
		nano::wallets wallets (*system.nodes[0]);
		ASSERT_EQ (1, wallets.wallet_count ());
		wallets.create (one);
		ASSERT_EQ (2, wallets.wallet_count ());
		wallets.destroy (one);
		ASSERT_EQ (1, wallets.wallet_count ());
		// give it some time so that the receivable blocks search can run
		std::this_thread::sleep_for (1000ms);
	}
	{
		nano::wallets wallets (*system.nodes[0]);
		ASSERT_EQ (1, wallets.wallet_count ());
		// give it some time so that the receivable blocks search can run
		std::this_thread::sleep_for (1000ms);
	}
}

// Opening multiple environments using the same file within the same process is not supported.
// http://www.lmdb.tech/doc/starting.html
TEST (wallets, DISABLED_reload)
{
	nano::test::system system (1);
	auto & node1 (*system.nodes[0]);
	nano::wallet_id one (1);
	ASSERT_EQ (1, node1.wallets.wallet_count ());
	{
		auto lock_wallet (node1.wallets.mutex.lock ());
		nano::node_flags flags{ nano::inactive_node_flag_defaults () };
		nano::inactive_node node (node1.application_path, flags);
		node.node->wallets.create (one);
	}
	ASSERT_TIMELY (5s, node1.wallets.wallet_exists (one));
	ASSERT_EQ (2, node1.wallets.wallet_count ());
}

TEST (wallets, vote_minimum)
{
	nano::test::system system (1);
	auto & node1 (*system.nodes[0]);
	nano::keypair key1;
	nano::keypair key2;
	nano::block_builder builder;
	auto send1 = builder
				 .state ()
				 .account (nano::dev::genesis_key.pub)
				 .previous (nano::dev::genesis->hash ())
				 .representative (nano::dev::genesis_key.pub)
				 .balance (std::numeric_limits<nano::uint128_t>::max () - node1.config->vote_minimum.number ())
				 .link (key1.pub)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (nano::dev::genesis->hash ()))
				 .build ();
	ASSERT_EQ (nano::block_status::progress, node1.process (send1));
	auto open1 = builder
				 .state ()
				 .account (key1.pub)
				 .previous (0)
				 .representative (key1.pub)
				 .balance (node1.config->vote_minimum.number ())
				 .link (send1->hash ())
				 .sign (key1.prv, key1.pub)
				 .work (*system.work.generate (key1.pub))
				 .build ();
	ASSERT_EQ (nano::block_status::progress, node1.process (open1));
	// send2 with amount vote_minimum - 1 (not voting representative)
	auto send2 = builder
				 .state ()
				 .account (nano::dev::genesis_key.pub)
				 .previous (send1->hash ())
				 .representative (nano::dev::genesis_key.pub)
				 .balance (std::numeric_limits<nano::uint128_t>::max () - 2 * node1.config->vote_minimum.number () + 1)
				 .link (key2.pub)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (send1->hash ()))
				 .build ();
	ASSERT_EQ (nano::block_status::progress, node1.process (send2));
	auto open2 = builder
				 .state ()
				 .account (key2.pub)
				 .previous (0)
				 .representative (key2.pub)
				 .balance (node1.config->vote_minimum.number () - 1)
				 .link (send2->hash ())
				 .sign (key2.prv, key2.pub)
				 .work (*system.work.generate (key2.pub))
				 .build ();
	ASSERT_EQ (nano::block_status::progress, node1.process (open2));
	auto wallet_id{ node1.wallets.first_wallet_id () };
	ASSERT_EQ (0, node1.wallets.representatives_count (wallet_id));
	(void)node1.wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);
	(void)node1.wallets.insert_adhoc (wallet_id, key1.prv);
	(void)node1.wallets.insert_adhoc (wallet_id, key2.prv);
	node1.wallets.compute_reps ();
	ASSERT_EQ (2, node1.wallets.representatives_count (wallet_id));
}

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
