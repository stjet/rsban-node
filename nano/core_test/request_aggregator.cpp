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

TEST (request_aggregator, channel_max_queue)
{
	nano::test::system system;
	nano::node_config node_config = system.default_config ();
	node_config.frontiers_confirmation = nano::frontiers_confirmation_mode::disabled;
	node_config.request_aggregator.max_queue = 0;
	auto & node (*system.add_node (node_config));
	(void)node.wallets.insert_adhoc (node.wallets.first_wallet_id (), nano::dev::genesis_key.prv);
	nano::block_builder builder;
	auto send1 = builder
				 .state ()
				 .account (nano::dev::genesis_key.pub)
				 .previous (nano::dev::genesis->hash ())
				 .representative (nano::dev::genesis_key.pub)
				 .balance (nano::dev::constants.genesis_amount - nano::Gxrb_ratio)
				 .link (nano::dev::genesis_key.pub)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*node.work_generate_blocking (nano::dev::genesis->hash ()))
				 .build ();
	ASSERT_EQ (nano::block_status::progress, node.ledger.process (*node.store.tx_begin_write (), send1));
	std::vector<std::pair<nano::block_hash, nano::root>> request;
	request.emplace_back (send1->hash (), send1->root ());
	auto client = nano::transport::create_client_socket (node);
	std::shared_ptr<nano::transport::channel> dummy_channel = create_dummy_channel (node, client);
	node.aggregator.request (request, dummy_channel);
	node.aggregator.request (request, dummy_channel);
	ASSERT_LT (0, node.stats->count (nano::stat::type::aggregator, nano::stat::detail::aggregator_dropped));
}

TEST (request_aggregator, cannot_vote)
{
	nano::test::system system;
	nano::node_flags flags;
	flags.set_disable_request_loop (true);
	auto & node (*system.add_node (flags));
	nano::state_block_builder builder;
	auto send1 = builder.make_block ()
				 .account (nano::dev::genesis_key.pub)
				 .previous (nano::dev::genesis->hash ())
				 .representative (nano::dev::genesis_key.pub)
				 .balance (nano::dev::constants.genesis_amount - 1)
				 .link (nano::dev::genesis_key.pub)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (nano::dev::genesis->hash ()))
				 .build ();
	auto send2 = builder.make_block ()
				 .from (*send1)
				 .previous (send1->hash ())
				 .balance (send1->balance_field ().value ().number () - 1)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*system.work.generate (send1->hash ()))
				 .build ();
	ASSERT_EQ (nano::block_status::progress, node.process (send1));
	ASSERT_EQ (nano::block_status::progress, node.process (send2));
	(void)node.wallets.insert_adhoc (node.wallets.first_wallet_id (), nano::dev::genesis_key.prv);
	ASSERT_FALSE (node.ledger.dependents_confirmed (*node.store.tx_begin_read (), *send2));

	std::vector<std::pair<nano::block_hash, nano::root>> request;
	// Correct hash, correct root
	request.emplace_back (send2->hash (), send2->root ());
	// Incorrect hash, correct root
	request.emplace_back (1, send2->root ());

	auto client = nano::transport::create_client_socket (node);
	std::shared_ptr<nano::transport::channel> dummy_channel = create_dummy_channel (node, client);
	node.aggregator.request (request, dummy_channel);
	ASSERT_TIMELY (3s, node.aggregator.empty ());
	ASSERT_EQ (1, node.stats->count (nano::stat::type::aggregator, nano::stat::detail::aggregator_accepted));
	ASSERT_EQ (0, node.stats->count (nano::stat::type::aggregator, nano::stat::detail::aggregator_dropped));
	ASSERT_TIMELY_EQ (3s, 2, node.stats->count (nano::stat::type::requests, nano::stat::detail::requests_non_final));
	ASSERT_EQ (0, node.stats->count (nano::stat::type::requests, nano::stat::detail::requests_generated_votes));
	ASSERT_EQ (0, node.stats->count (nano::stat::type::requests, nano::stat::detail::requests_unknown));
	ASSERT_EQ (0, node.stats->count (nano::stat::type::message, nano::stat::detail::confirm_ack, nano::stat::dir::out));

	// With an ongoing election
	node.start_election (send2);
	ASSERT_TIMELY (5s, node.active.election (send2->qualified_root ()));

	node.aggregator.request (request, dummy_channel);
	ASSERT_TIMELY (3s, node.aggregator.empty ());
	ASSERT_EQ (2, node.stats->count (nano::stat::type::aggregator, nano::stat::detail::aggregator_accepted));
	ASSERT_EQ (0, node.stats->count (nano::stat::type::aggregator, nano::stat::detail::aggregator_dropped));
	ASSERT_TIMELY_EQ (3s, 4, node.stats->count (nano::stat::type::requests, nano::stat::detail::requests_non_final));
	ASSERT_EQ (0, node.stats->count (nano::stat::type::requests, nano::stat::detail::requests_generated_votes));
	ASSERT_EQ (0, node.stats->count (nano::stat::type::requests, nano::stat::detail::requests_unknown));
	ASSERT_EQ (0, node.stats->count (nano::stat::type::message, nano::stat::detail::confirm_ack, nano::stat::dir::out));

	// Confirm send1 and send2
	nano::test::confirm (node.ledger, { send1, send2 });

	node.aggregator.request (request, dummy_channel);
	ASSERT_TIMELY (3s, node.aggregator.empty ());
	ASSERT_EQ (3, node.stats->count (nano::stat::type::aggregator, nano::stat::detail::aggregator_accepted));
	ASSERT_EQ (0, node.stats->count (nano::stat::type::aggregator, nano::stat::detail::aggregator_dropped));
	ASSERT_EQ (4, node.stats->count (nano::stat::type::requests, nano::stat::detail::requests_non_final));
	ASSERT_TIMELY_EQ (3s, 1, node.stats->count (nano::stat::type::requests, nano::stat::detail::requests_generated_hashes));
	ASSERT_TIMELY_EQ (3s, 1, node.stats->count (nano::stat::type::requests, nano::stat::detail::requests_generated_votes));
	ASSERT_EQ (0, node.stats->count (nano::stat::type::requests, nano::stat::detail::requests_unknown));
	ASSERT_TIMELY (3s, 1 <= node.stats->count (nano::stat::type::message, nano::stat::detail::confirm_ack, nano::stat::dir::out));
}
