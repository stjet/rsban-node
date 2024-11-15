#include <nano/lib/blocks.hpp>
#include <nano/node/active_elections.hpp>
#include <nano/node/election.hpp>
#include <nano/test_common/chains.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

#include <chrono>

using namespace std::chrono_literals;

/*
 * Ensure accounts with some blocks already confirmed and with less than `gap_threshold` blocks do not get activated
 */
TEST (optimistic_scheduler, under_gap_threshold)
{
	nano::test::system system{};
	nano::node_config config = system.default_config ();
	config.frontiers_confirmation = nano::frontiers_confirmation_mode::disabled;
	auto & node = *system.add_node (config);

	// Must be smaller than optimistic scheduler `gap_threshold`
	const int howmany_blocks = 64;

	auto chains = nano::test::setup_chains (system, node, /* single chain */ 1, howmany_blocks, nano::dev::genesis_key, /* do not confirm */ false);
	auto & [account, blocks] = chains.front ();

	// Confirm block towards the end of the chain, so gap between confirmation and account frontier is less than `gap_threshold`
	nano::test::confirm (node.ledger, blocks.at (55));

	// Manually trigger backlog scan
	node.backlog.trigger ();

	// Ensure unconfirmed account head block gets activated
	auto const & block = blocks.back ();
	ASSERT_NEVER (3s, node.election_active (block->hash ()));
}
