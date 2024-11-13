#include <nano/lib/blocks.hpp>
#include <nano/node/active_elections.hpp>
#include <nano/node/election.hpp>
#include <nano/node/scheduler/component.hpp>
#include <nano/node/scheduler/priority.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/test_common/chains.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

using namespace std::chrono_literals;

TEST (election, construction)
{
	nano::test::system system (1);
	auto & node = *system.nodes[0];
	auto election = std::make_shared<nano::election> (
	node, nano::dev::genesis, [] (auto const &) {}, [] (auto const &) {}, nano::election_behavior::priority);
}

TEST (election, behavior)
{
	nano::test::system system (1);
	auto chain = nano::test::setup_chain (system, *system.nodes[0], 1, nano::dev::genesis_key, false);
	auto election = nano::test::start_election (system, *system.nodes[0], chain[0]->hash ());
	ASSERT_NE (nullptr, election);
	ASSERT_EQ (nano::election_behavior::manual, election->behavior ());
}

