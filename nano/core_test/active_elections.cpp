#include <nano/lib/blocks.hpp>
#include <nano/lib/jsonconfig.hpp>
#include <nano/lib/numbers.hpp>
#include <nano/node/active_elections.hpp>
#include <nano/node/election.hpp>
#include <nano/node/scheduler/component.hpp>
#include <nano/node/scheduler/manual.hpp>
#include <nano/node/scheduler/priority.hpp>
#include <nano/secure/common.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/test_common/chains.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

#include <thread>

using namespace std::chrono_literals;

TEST (active_elections, activate_inactive)
{
	nano::test::system system;
	nano::node_flags flags;
	nano::node_config config = system.default_config ();
	config.frontiers_confirmation = nano::frontiers_confirmation_mode::disabled;
	auto & node = *system.add_node (config, flags);

	nano::keypair key;
	nano::state_block_builder builder;
	auto send = builder.make_block ()
				.account (nano::dev::genesis_key.pub)
				.previous (nano::dev::genesis->hash ())
				.representative (nano::dev::genesis_key.pub)
				.link (key.pub)
				.balance (nano::dev::constants.genesis_amount - 1)
				.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				.work (*system.work.generate (nano::dev::genesis->hash ()))
				.build ();
	auto send2 = builder.make_block ()
				 .account (nano::dev::genesis_key.pub)
				 .previous (send->hash ())
				 .representative (nano::dev::genesis_key.pub)
				 .link (nano::keypair ().pub)
				 .balance (nano::dev::constants.genesis_amount - 2)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (send->hash ()))
				 .build ();
	auto open = builder.make_block ()
				.account (key.pub)
				.previous (0)
				.representative (key.pub)
				.link (send->hash ())
				.balance (1)
				.sign (key.prv, key.pub)
				.work (*system.work.generate (key.pub))
				.build ();

	ASSERT_EQ (nano::block_status::progress, node.process (send));
	ASSERT_EQ (nano::block_status::progress, node.process (send2));
	ASSERT_EQ (nano::block_status::progress, node.process (open));

	auto election = nano::test::start_election (system, node, send2->hash ());
	ASSERT_NE (nullptr, election);
	node.active.force_confirm (*election);

	ASSERT_TIMELY (5s, !node.confirming_set.exists (send2->hash ()));
	ASSERT_TIMELY (5s, node.block_confirmed (send2->hash ()));
	ASSERT_TIMELY (5s, node.block_confirmed (send->hash ()));

	// wait so that blocks observer can increase the stats
	std::this_thread::sleep_for (1000ms);

	ASSERT_TIMELY_EQ (5s, 1, node.stats->count (nano::stat::type::confirmation_observer, nano::stat::detail::inactive_conf_height, nano::stat::dir::out));
	ASSERT_TIMELY_EQ (5s, 1, node.stats->count (nano::stat::type::confirmation_observer, nano::stat::detail::active_quorum, nano::stat::dir::out));
	ASSERT_ALWAYS_EQ (50ms, 0, node.stats->count (nano::stat::type::confirmation_observer, nano::stat::detail::active_conf_height, nano::stat::dir::out));

	// The first block was not active so no activation takes place
	ASSERT_FALSE (node.active.active (open->qualified_root ()) || node.block_confirmed_or_being_confirmed (open->hash ()));
}

/*
 * Ensures we limit the number of vote hinted elections in AEC
 */
// disabled because it doesn't run after tokio switch
TEST (DISABLED_active_elections, limit_vote_hinted_elections)
{
	// TODO reimplement in Rust
}
