#include <nano/lib/blocks.hpp>
#include <nano/lib/jsonconfig.hpp>
#include <nano/node/active_elections.hpp>
#include <nano/node/election.hpp>
#include <nano/node/local_vote_history.hpp>
#include <nano/node/request_aggregator.hpp>
#include <nano/node/transport/inproc.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/test_common/network.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

using namespace std::chrono_literals;

std::shared_ptr<nano::transport::channel> create_dummy_channel (nano::node & node, std::shared_ptr<nano::transport::socket> client)
{
	return std::make_shared<nano::transport::channel_tcp> (
	node.async_rt,
	node.outbound_limiter,
	node.network_params.network,
	client,
	*node.stats,
	*node.network->tcp_channels,
	1);
}

