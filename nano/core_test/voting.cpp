#include <nano/lib/blocks.hpp>
#include <nano/node/common.hpp>
#include <nano/node/local_vote_history.hpp>
#include <nano/node/vote_generator.hpp>
#include <nano/node/vote_spacing.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

using namespace std::chrono_literals;

TEST (vote_generator, cache)
{
	nano::test::system system (1);
	auto & node (*system.nodes[0]);
	auto epoch1 = system.upgrade_genesis_epoch (node, nano::epoch::epoch_1);
	(void)node.wallets.insert_adhoc (node.wallets.first_wallet_id (), nano::dev::genesis_key.prv);
	node.generator.add (epoch1->root (), epoch1->hash ());
	ASSERT_TIMELY (1s, !node.history.votes (epoch1->root (), epoch1->hash ()).empty ());
	auto votes (node.history.votes (epoch1->root (), epoch1->hash ()));
	ASSERT_FALSE (votes.empty ());
	auto hashes{ votes[0]->hashes () };
	ASSERT_TRUE (std::any_of (hashes.begin (), hashes.end (), [hash = epoch1->hash ()] (nano::block_hash const & hash_a) { return hash_a == hash; }));
}

TEST (vote_generator, multiple_representatives)
{
	nano::test::system system (1);
	auto & node (*system.nodes[0]);
	auto wallet_id = node.wallets.first_wallet_id ();
	nano::keypair key1, key2, key3;
	(void)node.wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);
	(void)node.wallets.insert_adhoc (wallet_id, key1.prv);
	(void)node.wallets.insert_adhoc (wallet_id, key2.prv);
	(void)node.wallets.insert_adhoc (wallet_id, key3.prv);
	auto const amount = 100 * nano::Gxrb_ratio;
	node.wallets.send_sync (wallet_id, nano::dev::genesis_key.pub, key1.pub, amount);
	node.wallets.send_sync (wallet_id, nano::dev::genesis_key.pub, key2.pub, amount);
	node.wallets.send_sync (wallet_id, nano::dev::genesis_key.pub, key3.pub, amount);
	ASSERT_TIMELY (3s, node.balance (key1.pub) == amount && node.balance (key2.pub) == amount && node.balance (key3.pub) == amount);
	node.wallets.change_sync (wallet_id, key1.pub, key1.pub);
	node.wallets.change_sync (wallet_id, key2.pub, key2.pub);
	node.wallets.change_sync (wallet_id, key3.pub, key3.pub);
	ASSERT_EQ (node.weight (key1.pub), amount);
	ASSERT_EQ (node.weight (key2.pub), amount);
	ASSERT_EQ (node.weight (key3.pub), amount);
	node.wallets.compute_reps ();
	ASSERT_EQ (4, node.wallets.voting_reps_count ());
	auto hash = node.wallets.send_sync (wallet_id, nano::dev::genesis_key.pub, nano::dev::genesis_key.pub, 1);
	auto send = node.block (hash);
	ASSERT_NE (nullptr, send);
	ASSERT_TIMELY_EQ (5s, node.history.votes (send->root (), send->hash ()).size (), 4);
	auto votes (node.history.votes (send->root (), send->hash ()));
	for (auto const & account : { key1.pub, key2.pub, key3.pub, nano::dev::genesis_key.pub })
	{
		auto existing (std::find_if (votes.begin (), votes.end (), [&account] (std::shared_ptr<nano::vote> const & vote_a) -> bool {
			return vote_a->account () == account;
		}));
		ASSERT_NE (votes.end (), existing);
	}
}

TEST (vote_spacing, basic)
{
	nano::vote_spacing spacing{ std::chrono::milliseconds{ 100 } };
	nano::root root1{ 1 };
	nano::root root2{ 2 };
	nano::block_hash hash3{ 3 };
	nano::block_hash hash4{ 4 };
	nano::block_hash hash5{ 5 };
	ASSERT_EQ (0, spacing.size ());
	ASSERT_TRUE (spacing.votable (root1, hash3));
	spacing.flag (root1, hash3);
	ASSERT_EQ (1, spacing.size ());
	ASSERT_TRUE (spacing.votable (root1, hash3));
	ASSERT_FALSE (spacing.votable (root1, hash4));
	spacing.flag (root2, hash5);
	ASSERT_EQ (2, spacing.size ());
}

TEST (vote_spacing, prune)
{
	auto length = std::chrono::milliseconds{ 100 };
	nano::vote_spacing spacing{ length };
	nano::root root1{ 1 };
	nano::root root2{ 2 };
	nano::block_hash hash3{ 3 };
	nano::block_hash hash4{ 4 };
	spacing.flag (root1, hash3);
	ASSERT_EQ (1, spacing.size ());
	std::this_thread::sleep_for (length);
	spacing.flag (root2, hash4);
	ASSERT_EQ (1, spacing.size ());
}

TEST (vote_spacing, vote_generator)
{
	nano::node_config config;
	config.frontiers_confirmation = nano::frontiers_confirmation_mode::disabled;
	config.active_elections.hinted_limit_percentage = 0; // Disable election hinting
	nano::test::system system;
	nano::node_flags node_flags;
	node_flags.set_disable_search_pending (true);
	auto & node = *system.add_node (config, node_flags);
	(void)node.wallets.insert_adhoc (node.wallets.first_wallet_id (), nano::dev::genesis_key.prv);
	nano::state_block_builder builder;
	auto send1 = builder.make_block ()
				 .account (nano::dev::genesis_key.pub)
				 .previous (nano::dev::genesis->hash ())
				 .representative (nano::dev::genesis_key.pub)
				 .balance (nano::dev::constants.genesis_amount - nano::Gxrb_ratio)
				 .link (nano::dev::genesis_key.pub)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (nano::dev::genesis->hash ()))
				 .build ();
	auto send2 = builder.make_block ()
				 .account (nano::dev::genesis_key.pub)
				 .previous (nano::dev::genesis->hash ())
				 .representative (nano::dev::genesis_key.pub)
				 .balance (nano::dev::constants.genesis_amount - nano::Gxrb_ratio - 1)
				 .link (nano::dev::genesis_key.pub)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (nano::dev::genesis->hash ()))
				 .build ();
	ASSERT_EQ (nano::block_status::progress, node.ledger.process (*node.store.tx_begin_write (), send1));
	ASSERT_EQ (0, node.stats->count (nano::stat::type::vote_generator, nano::stat::detail::generator_broadcasts));
	node.generator.add (nano::dev::genesis->hash (), send1->hash ());
	ASSERT_TIMELY_EQ (3s, node.stats->count (nano::stat::type::vote_generator, nano::stat::detail::generator_broadcasts), 1);
	ASSERT_FALSE (node.ledger.rollback (*node.store.tx_begin_write (), send1->hash ()));
	ASSERT_EQ (nano::block_status::progress, node.ledger.process (*node.store.tx_begin_write (), send2));
	node.generator.add (nano::dev::genesis->hash (), send2->hash ());
	ASSERT_TIMELY_EQ (3s, node.stats->count (nano::stat::type::vote_generator, nano::stat::detail::generator_spacing), 1);
	ASSERT_EQ (1, node.stats->count (nano::stat::type::vote_generator, nano::stat::detail::generator_broadcasts));
	std::this_thread::sleep_for (config.network_params.voting.delay);
	node.generator.add (nano::dev::genesis->hash (), send2->hash ());
	ASSERT_TIMELY_EQ (3s, node.stats->count (nano::stat::type::vote_generator, nano::stat::detail::generator_broadcasts), 2);
}

TEST (vote_spacing, rapid)
{
	nano::node_config config;
	config.frontiers_confirmation = nano::frontiers_confirmation_mode::disabled;
	config.active_elections.hinted_limit_percentage = 0; // Disable election hinting
	nano::test::system system;
	nano::node_flags node_flags;
	node_flags.set_disable_search_pending (true);
	auto & node = *system.add_node (config, node_flags);
	(void)node.wallets.insert_adhoc (node.wallets.first_wallet_id (), nano::dev::genesis_key.prv);
	nano::state_block_builder builder;
	auto send1 = builder.make_block ()
				 .account (nano::dev::genesis_key.pub)
				 .previous (nano::dev::genesis->hash ())
				 .representative (nano::dev::genesis_key.pub)
				 .balance (nano::dev::constants.genesis_amount - nano::Gxrb_ratio)
				 .link (nano::dev::genesis_key.pub)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (nano::dev::genesis->hash ()))
				 .build ();
	auto send2 = builder.make_block ()
				 .account (nano::dev::genesis_key.pub)
				 .previous (nano::dev::genesis->hash ())
				 .representative (nano::dev::genesis_key.pub)
				 .balance (nano::dev::constants.genesis_amount - nano::Gxrb_ratio - 1)
				 .link (nano::dev::genesis_key.pub)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (nano::dev::genesis->hash ()))
				 .build ();
	ASSERT_EQ (nano::block_status::progress, node.ledger.process (*node.store.tx_begin_write (), send1));
	node.generator.add (nano::dev::genesis->hash (), send1->hash ());
	ASSERT_TIMELY_EQ (3s, node.stats->count (nano::stat::type::vote_generator, nano::stat::detail::generator_broadcasts), 1);
	ASSERT_FALSE (node.ledger.rollback (*node.store.tx_begin_write (), send1->hash ()));
	ASSERT_EQ (nano::block_status::progress, node.ledger.process (*node.store.tx_begin_write (), send2));
	node.generator.add (nano::dev::genesis->hash (), send2->hash ());
	ASSERT_TIMELY_EQ (3s, node.stats->count (nano::stat::type::vote_generator, nano::stat::detail::generator_spacing), 1);
	ASSERT_TIMELY_EQ (3s, 1, node.stats->count (nano::stat::type::vote_generator, nano::stat::detail::generator_broadcasts));
	std::this_thread::sleep_for (config.network_params.voting.delay);
	node.generator.add (nano::dev::genesis->hash (), send2->hash ());
	ASSERT_TIMELY_EQ (3s, node.stats->count (nano::stat::type::vote_generator, nano::stat::detail::generator_broadcasts), 2);
}
