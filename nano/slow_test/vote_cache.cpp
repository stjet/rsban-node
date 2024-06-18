#include <nano/lib/blocks.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

#include <functional>
#include <thread>

using namespace std::chrono_literals;

namespace
{
nano::keypair setup_rep (nano::test::system & system, nano::node & node, nano::uint128_t amount)
{
	auto latest = node.latest (nano::dev::genesis_key.pub);
	auto balance = node.balance (nano::dev::genesis_key.pub);

	nano::keypair key;
	nano::block_builder builder;

	auto send = builder
				.send ()
				.previous (latest)
				.destination (key.pub)
				.balance (balance - amount)
				.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				.work (*system.work.generate (latest))
				.build ();

	auto open = builder
				.open ()
				.source (send->hash ())
				.representative (key.pub)
				.account (key.pub)
				.sign (key.prv, key.pub)
				.work (*system.work.generate (key.pub))
				.build ();

	EXPECT_TRUE (nano::test::process (node, { send, open }));
	nano::test::confirm (node.ledger, open->hash ());

	return key;
}

std::vector<nano::keypair> setup_reps (nano::test::system & system, nano::node & node, int count)
{
	const nano::uint128_t weight = nano::Gxrb_ratio * 1000;
	std::vector<nano::keypair> reps;
	for (int n = 0; n < count; ++n)
	{
		reps.push_back (setup_rep (system, node, weight));
	}
	return reps;
}

/*
 * Creates `count` number of unconfirmed blocks with their dependencies confirmed, each directly sent from genesis
 */
std::vector<std::shared_ptr<nano::block>> setup_blocks (nano::test::system & system, nano::node & node, int count)
{
	auto latest = node.latest (nano::dev::genesis_key.pub);
	auto balance = node.balance (nano::dev::genesis_key.pub);

	std::vector<std::shared_ptr<nano::block>> sends;
	std::vector<std::shared_ptr<nano::block>> receives;
	for (int n = 0; n < count; ++n)
	{
		if (n % 10000 == 0)
			std::cout << "setup_blocks: " << n << std::endl;

		nano::keypair key;
		nano::block_builder builder;

		balance -= 1;
		auto send = builder
					.send ()
					.previous (latest)
					.destination (key.pub)
					.balance (balance)
					.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
					.work (*system.work.generate (latest))
					.build ();

		auto open = builder
					.open ()
					.source (send->hash ())
					.representative (key.pub)
					.account (key.pub)
					.sign (key.prv, key.pub)
					.work (*system.work.generate (key.pub))
					.build ();

		latest = send->hash ();

		sends.push_back (send);
		receives.push_back (open);
	}

	std::cout << "setup_blocks confirming" << std::endl;

	EXPECT_TRUE (nano::test::process (node, sends));
	EXPECT_TRUE (nano::test::process (node, receives));

	// Confirm whole genesis chain at once
	nano::test::confirm (node.ledger, sends.back ()->hash ());

	std::cout << "setup_blocks done" << std::endl;

	return receives;
}

void run_parallel (int thread_count, std::function<void (int)> func)
{
	std::vector<std::thread> threads;
	for (int n = 0; n < thread_count; ++n)
	{
		threads.emplace_back ([func, n] () {
			func (n);
		});
	}
	for (auto & thread : threads)
	{
		thread.join ();
	}
}
}
