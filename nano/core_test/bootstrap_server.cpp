#include <nano/lib/blocks.hpp>
#include <nano/test_common/chains.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

#include <iterator>

using namespace std::chrono_literals;

namespace
{
class responses_helper final
{
public:
	void add (nano::asc_pull_ack const & ack)
	{
		nano::lock_guard<nano::mutex> lock{ mutex };
		responses.push_back (ack);
	}

	std::vector<nano::asc_pull_ack> get ()
	{
		nano::lock_guard<nano::mutex> lock{ mutex };
		return responses;
	}

	std::size_t size ()
	{
		nano::lock_guard<nano::mutex> lock{ mutex };
		return responses.size ();
	}

	void connect (nano::bootstrap_server & server)
	{
		server.set_response_callback ([&] (auto & response, auto & channel) {
			add (response);
		});
	}

private:
	nano::mutex mutex;
	std::vector<nano::asc_pull_ack> responses;
};

/**
 * Checks if both lists contain the same blocks, with `blocks_b` skipped by `skip` elements
 */
bool compare_blocks (std::vector<std::shared_ptr<nano::block>> blocks_a, std::vector<std::shared_ptr<nano::block>> blocks_b, int skip = 0)
{
	debug_assert (blocks_b.size () >= blocks_a.size () + skip);

	const auto count = blocks_a.size ();
	for (int n = 0; n < count; ++n)
	{
		auto & block_a = *blocks_a[n];
		auto & block_b = *blocks_b[n + skip];

		// nano::block does not have != operator
		if (!(block_a == block_b))
		{
			return false;
		}
	}
	return true;
}
}

TEST (bootstrap_server, serve_frontiers_invalid_count)
{
	nano::test::system system{};
	auto & node = *system.add_node ();

	responses_helper responses;
	responses.connect (node.bootstrap_server);

	auto chains = nano::test::setup_chains (system, node, /* chain count */ 4, /* block count */ 4);

	// Zero count
	{
		nano::asc_pull_req::frontiers_payload request_payload{};
		request_payload.count = 0;
		request_payload.start = 0;
		nano::asc_pull_req request{ node.network_params.network, 7, request_payload };

		node.network->inbound (request, nano::test::fake_channel (node));
	}

	ASSERT_TIMELY_EQ (5s, node.stats->count (nano::stat::type::bootstrap_server, nano::stat::detail::invalid), 1);

	// Count larger than allowed
	{
		nano::asc_pull_req::frontiers_payload request_payload{};
		request_payload.count = nano::bootstrap_server::max_frontiers + 1;
		request_payload.start = 0;
		nano::asc_pull_req request{ node.network_params.network, 7, request_payload };

		node.network->inbound (request, nano::test::fake_channel (node));
	}

	ASSERT_TIMELY_EQ (5s, node.stats->count (nano::stat::type::bootstrap_server, nano::stat::detail::invalid), 2);

	// Max numeric value
	{
		nano::asc_pull_req::frontiers_payload request_payload{};
		request_payload.count = std::numeric_limits<decltype (request_payload.count)>::max ();
		request_payload.start = 0;
		nano::asc_pull_req request{ node.network_params.network, 7, request_payload };

		node.network->inbound (request, nano::test::fake_channel (node));
	}

	ASSERT_TIMELY_EQ (5s, node.stats->count (nano::stat::type::bootstrap_server, nano::stat::detail::invalid), 3);

	// Ensure we don't get any unexpected responses
	ASSERT_ALWAYS (1s, responses.size () == 0);
}
