#include <nano/test_common/system.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

#include <thread>

using namespace std::chrono_literals;

TEST (distributed_work, no_peers)
{
	nano::test::system system (1);
	auto node (system.nodes[0]);
	nano::block_hash hash{ 1 };
	std::optional<uint64_t> work;
	std::atomic<bool> done{ false };
	auto callback = [&work, &done] (std::optional<uint64_t> work_a) {
		ASSERT_TRUE (work_a.has_value ());
		work = work_a;
		done = true;
	};
	node->distributed_work.make (nano::work_version::work_1, hash, node->config->work_peers, node->network_params.work.get_base (), callback, nano::account ());
	ASSERT_TIMELY (5s, done);
	ASSERT_GE (nano::dev::network_params.work.difficulty (nano::work_version::work_1, hash, *work), node->network_params.work.get_base ());
}

TEST (distributed_work, no_peers_disabled)
{
	nano::test::system system{ nano::work_generation::disabled };
	nano::node_config node_config = system.default_config ();
	node_config.work_threads = 0;
	auto & node = *system.add_node (node_config);
	node.distributed_work.make (nano::work_version::work_1, nano::block_hash (), node.config->work_peers, nano::dev::network_params.work.get_base (), {});
}

TEST (distributed_work, no_peers_cancel)
{
	nano::test::system system;
	nano::node_config node_config = system.default_config ();
	node_config.max_work_generate_multiplier = 1e6;
	auto & node = *system.add_node (node_config);
	nano::block_hash hash{ 1 };
	bool done{ false };
	auto callback_to_cancel = [&done] (std::optional<uint64_t> work_a) {
		ASSERT_FALSE (work_a.has_value ());
		done = true;
	};
	node.distributed_work.make (nano::work_version::work_1, hash, node.config->work_peers, nano::difficulty::from_multiplier (1e6, node.network_params.work.get_base ()), callback_to_cancel);

	std::this_thread::sleep_for (100ms);

	// manually cancel
	node.distributed_work.cancel (hash);
	ASSERT_TIMELY (20s, done);
}

TEST (distributed_work, no_peers_multi)
{
	nano::test::system system (1);
	auto node (system.nodes[0]);
	nano::block_hash hash{ 1 };
	unsigned total{ 10 };
	std::atomic<unsigned> count{ 0 };
	auto callback = [&count] (std::optional<uint64_t> work_a) {
		ASSERT_TRUE (work_a.has_value ());
		++count;
	};
	// Test many works for the same root
	for (unsigned i{ 0 }; i < total; ++i)
	{
		node->distributed_work.make (nano::work_version::work_1, hash, node->config->work_peers, nano::difficulty::from_multiplier (10, node->network_params.work.get_base ()), callback);
	}
	ASSERT_TIMELY_EQ (5s, count, total);
	count = 0;
	// Test many works for different roots
	for (unsigned i{ 0 }; i < total; ++i)
	{
		nano::block_hash hash_i (i + 1);
		node->distributed_work.make (nano::work_version::work_1, hash_i, node->config->work_peers, node->network_params.work.get_base (), callback);
	}
	ASSERT_TIMELY_EQ (5s, count, total);
}
