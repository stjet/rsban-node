#include <nano/lib/blocks.hpp>
#include <nano/node/common.hpp>
#include <nano/node/local_vote_history.hpp>
#include <nano/node/vote_spacing.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

using namespace std::chrono_literals;

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
	node.enqueue_vote_request (nano::dev::genesis->hash (), send1->hash ());
	ASSERT_TIMELY_EQ (3s, node.stats->count (nano::stat::type::vote_generator, nano::stat::detail::generator_broadcasts), 1);
	ASSERT_FALSE (node.ledger.rollback (*node.store.tx_begin_write (), send1->hash ()));
	ASSERT_EQ (nano::block_status::progress, node.ledger.process (*node.store.tx_begin_write (), send2));
	node.enqueue_vote_request (nano::dev::genesis->hash (), send2->hash ());
	ASSERT_TIMELY_EQ (3s, node.stats->count (nano::stat::type::vote_generator, nano::stat::detail::generator_spacing), 1);
	ASSERT_EQ (1, node.stats->count (nano::stat::type::vote_generator, nano::stat::detail::generator_broadcasts));
	std::this_thread::sleep_for (config.network_params.voting.delay);
	node.enqueue_vote_request (nano::dev::genesis->hash (), send2->hash ());
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
	node.enqueue_vote_request (nano::dev::genesis->hash (), send1->hash ());
	ASSERT_TIMELY_EQ (3s, node.stats->count (nano::stat::type::vote_generator, nano::stat::detail::generator_broadcasts), 1);
	ASSERT_FALSE (node.ledger.rollback (*node.store.tx_begin_write (), send1->hash ()));
	ASSERT_EQ (nano::block_status::progress, node.ledger.process (*node.store.tx_begin_write (), send2));
	node.enqueue_vote_request (nano::dev::genesis->hash (), send2->hash ());
	ASSERT_TIMELY_EQ (3s, node.stats->count (nano::stat::type::vote_generator, nano::stat::detail::generator_spacing), 1);
	ASSERT_TIMELY_EQ (3s, 1, node.stats->count (nano::stat::type::vote_generator, nano::stat::detail::generator_broadcasts));
	std::this_thread::sleep_for (config.network_params.voting.delay);
	node.enqueue_vote_request (nano::dev::genesis->hash (), send2->hash ());
	ASSERT_TIMELY_EQ (3s, node.stats->count (nano::stat::type::vote_generator, nano::stat::detail::generator_broadcasts), 2);
}
