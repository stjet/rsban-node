#include <nano/boost/beast/core/flat_buffer.hpp>
#include <nano/boost/beast/http.hpp>
#include <nano/lib/blocks.hpp>
#include <nano/lib/rpcconfig.hpp>
#include <nano/lib/thread_runner.hpp>
#include <nano/lib/threading.hpp>
#include <nano/node/active_elections.hpp>
#include <nano/node/election.hpp>
#include <nano/node/ipc/ipc_server.hpp>
#include <nano/node/json_handler.hpp>
#include <nano/node/node_rpc_config.hpp>
#include <nano/node/scheduler/component.hpp>
#include <nano/node/scheduler/manual.hpp>
#include <nano/node/scheduler/priority.hpp>
#include <nano/rpc/rpc.hpp>
#include <nano/rpc/rpc_request_processor.hpp>
#include <nano/rpc_test/common.hpp>
#include <nano/rpc_test/rpc_context.hpp>
#include <nano/rpc_test/test_response.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/test_common/chains.hpp>
#include <nano/test_common/network.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/telemetry.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

#include <boost/property_tree/json_parser.hpp>

#include <algorithm>
#include <future>
#include <map>
#include <thread>
#include <tuple>
#include <utility>

using namespace std::chrono_literals;
using namespace nano::test;

TEST (rpc, creation)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	ASSERT_NO_THROW (add_rpc (system, node));
}

TEST (rpc, wrapped_task)
{
	nano::test::system system;
	auto & node = *add_ipc_enabled_node (system);
	nano::node_rpc_config node_rpc_config;
	std::atomic<bool> response (false);
	auto response_handler_l ([&response] (std::string const & response_a) {
		std::stringstream istream (response_a);
		boost::property_tree::ptree json_l;
		ASSERT_NO_THROW (boost::property_tree::read_json (istream, json_l));
		ASSERT_EQ (1, json_l.count ("error"));
		ASSERT_EQ ("Unable to parse JSON", json_l.get<std::string> ("error"));
		response = true;
	});
	auto handler_l (std::make_shared<nano::json_handler> (node, node_rpc_config, "", response_handler_l));
	auto task (handler_l->create_worker_task ([] (std::shared_ptr<nano::json_handler> const &) {
		// Exception should get caught
		throw std::runtime_error ("");
	}));
	system.nodes[0]->workers->push_task (task);
	ASSERT_TIMELY_EQ (5s, response, true);
}

TEST (rpc, account_balance)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);

	// Add a send block (which will add a pending entry too) for the genesis account
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

	ASSERT_EQ (nano::block_status::progress, node->process (send1));
	ASSERT_TIMELY (5s, !node->active.active (*send1));

	auto const rpc_ctx = add_rpc (system, node);

	boost::property_tree::ptree request;
	request.put ("action", "account_balance");
	request.put ("account", nano::dev::genesis_key.pub.to_account ());

	// The send and pending should be unconfirmed
	{
		auto response (wait_response (system, rpc_ctx, request));
		std::string balance_text (response.get<std::string> ("balance"));
		ASSERT_EQ ("340282366920938463463374607431768211455", balance_text);
		std::string pending_text (response.get<std::string> ("pending"));
		ASSERT_EQ ("0", pending_text);
	}

	request.put ("include_only_confirmed", false);
	{
		auto response (wait_response (system, rpc_ctx, request));
		std::string balance_text (response.get<std::string> ("balance"));
		ASSERT_EQ ("340282366920938463463374607431768211454", balance_text);
		std::string pending_text (response.get<std::string> ("pending"));
		ASSERT_EQ ("1", pending_text);
	}
}

TEST (rpc, account_block_count)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	request.put ("action", "account_block_count");
	request.put ("account", nano::dev::genesis_key.pub.to_account ());
	auto response (wait_response (system, rpc_ctx, request));
	std::string block_count_text (response.get<std::string> ("block_count"));
	ASSERT_EQ ("1", block_count_text);
}

TEST (rpc, account_create)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	request.put ("action", "account_create");
	request.put ("wallet", node->wallets.first_wallet_id ().to_string ());
	auto response0 (wait_response (system, rpc_ctx, request));
	auto account_text0 (response0.get<std::string> ("account"));
	nano::account account0;
	ASSERT_FALSE (account0.decode_account (account_text0));
	ASSERT_TRUE (node->wallets.exists (account0));
	constexpr uint64_t max_index (std::numeric_limits<uint32_t>::max ());
	request.put ("index", max_index);
	auto response1 (wait_response (system, rpc_ctx, request, 10s));
	auto account_text1 (response1.get<std::string> ("account"));
	nano::account account1;
	ASSERT_FALSE (account1.decode_account (account_text1));
	ASSERT_TRUE (node->wallets.exists (account1));
	request.put ("index", max_index + 1);
	auto response2 (wait_response (system, rpc_ctx, request));
	ASSERT_EQ (std::error_code (nano::error_common::invalid_index).message (), response2.get<std::string> ("error"));
}

TEST (rpc, account_weight)
{
	nano::keypair key;
	nano::test::system system;
	auto node1 = add_ipc_enabled_node (system);
	nano::block_hash latest (node1->latest (nano::dev::genesis_key.pub));
	nano::block_builder builder;
	auto block = builder
				 .change ()
				 .previous (latest)
				 .representative (key.pub)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*node1->work_generate_blocking (latest))
				 .build ();
	ASSERT_EQ (nano::block_status::progress, node1->process (block));
	auto const rpc_ctx = add_rpc (system, node1);
	boost::property_tree::ptree request;
	request.put ("action", "account_weight");
	request.put ("account", key.pub.to_account ());
	auto response (wait_response (system, rpc_ctx, request));
	std::string balance_text (response.get<std::string> ("weight"));
	ASSERT_EQ ("340282366920938463463374607431768211455", balance_text);
}

TEST (rpc, wallet_contains)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	(void)node->wallets.insert_adhoc (node->wallets.first_wallet_id (), nano::dev::genesis_key.prv);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	std::string wallet;
	node->wallets.first_wallet_id ().encode_hex (wallet);
	request.put ("wallet", wallet);
	request.put ("action", "wallet_contains");
	request.put ("account", nano::dev::genesis_key.pub.to_account ());
	auto response (wait_response (system, rpc_ctx, request));
	std::string exists_text (response.get<std::string> ("exists"));
	ASSERT_EQ ("1", exists_text);
}

TEST (rpc, wallet_doesnt_contain)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	std::string wallet;
	node->wallets.first_wallet_id ().encode_hex (wallet);
	request.put ("wallet", wallet);
	request.put ("action", "wallet_contains");
	request.put ("account", nano::dev::genesis_key.pub.to_account ());
	auto response (wait_response (system, rpc_ctx, request));
	std::string exists_text (response.get<std::string> ("exists"));
	ASSERT_EQ ("0", exists_text);
}

TEST (rpc, validate_account_number)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	request.put ("action", "validate_account_number");
	request.put ("account", nano::dev::genesis_key.pub.to_account ());
	auto response (wait_response (system, rpc_ctx, request));
	std::string exists_text (response.get<std::string> ("valid"));
	ASSERT_EQ ("1", exists_text);
}

TEST (rpc, validate_account_invalid)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node);
	std::string account;
	nano::dev::genesis_key.pub.encode_account (account);
	account[0] ^= 0x1;
	boost::property_tree::ptree request;
	request.put ("action", "validate_account_number");
	request.put ("account", account);
	auto response (wait_response (system, rpc_ctx, request));
	std::string exists_text (response.get<std::string> ("valid"));
	ASSERT_EQ ("0", exists_text);
}

TEST (rpc, send)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	(void)node->wallets.insert_adhoc (node->wallets.first_wallet_id (), nano::dev::genesis_key.prv);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	std::string wallet;
	node->wallets.first_wallet_id ().encode_hex (wallet);
	request.put ("wallet", wallet);
	request.put ("action", "send");
	request.put ("source", nano::dev::genesis_key.pub.to_account ());
	request.put ("destination", nano::dev::genesis_key.pub.to_account ());
	request.put ("amount", "100");
	ASSERT_EQ (node->balance (nano::dev::genesis_key.pub), nano::dev::constants.genesis_amount);
	auto response (wait_response (system, rpc_ctx, request, 10s));
	std::string block_text (response.get<std::string> ("block"));
	nano::block_hash block;
	ASSERT_FALSE (block.decode_hex (block_text));
	ASSERT_TRUE (node->block_or_pruned_exists (block));
	ASSERT_EQ (node->latest (nano::dev::genesis_key.pub), block);
	ASSERT_NE (node->balance (nano::dev::genesis_key.pub), nano::dev::constants.genesis_amount);
}

TEST (rpc, send_fail)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	std::string wallet;
	node->wallets.first_wallet_id ().encode_hex (wallet);
	request.put ("wallet", wallet);
	request.put ("action", "send");
	request.put ("source", nano::dev::genesis_key.pub.to_account ());
	request.put ("destination", nano::dev::genesis_key.pub.to_account ());
	request.put ("amount", "100");
	auto response (wait_response (system, rpc_ctx, request, 10s));
	ASSERT_EQ (std::error_code (nano::error_common::account_not_found_wallet).message (), response.get<std::string> ("error"));
}

TEST (rpc, send_work)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	(void)node->wallets.insert_adhoc (node->wallets.first_wallet_id (), nano::dev::genesis_key.prv);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	std::string wallet;
	node->wallets.first_wallet_id ().encode_hex (wallet);
	request.put ("wallet", wallet);
	request.put ("action", "send");
	request.put ("source", nano::dev::genesis_key.pub.to_account ());
	request.put ("destination", nano::dev::genesis_key.pub.to_account ());
	request.put ("amount", "100");
	request.put ("work", "1");
	auto response (wait_response (system, rpc_ctx, request, 10s));
	ASSERT_EQ (std::error_code (nano::error_common::invalid_work).message (), response.get<std::string> ("error"));
	request.erase ("work");
	request.put ("work", nano::to_string_hex (*node->work_generate_blocking (node->latest (nano::dev::genesis_key.pub))));
	auto response2 (wait_response (system, rpc_ctx, request, 10s));
	std::string block_text (response2.get<std::string> ("block"));
	nano::block_hash block;
	ASSERT_FALSE (block.decode_hex (block_text));
	ASSERT_TRUE (node->block_or_pruned_exists (block));
	ASSERT_EQ (node->latest (nano::dev::genesis_key.pub), block);
}

TEST (rpc, send_work_disabled)
{
	nano::test::system system{ nano::work_generation::disabled };
	nano::node_config node_config = system.default_config ();
	node_config.work_threads = 0;
	auto node = add_ipc_enabled_node (system, node_config);
	(void)node->wallets.insert_adhoc (node->wallets.first_wallet_id (), nano::dev::genesis_key.prv);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	std::string wallet;
	node->wallets.first_wallet_id ().encode_hex (wallet);
	request.put ("wallet", wallet);
	request.put ("action", "send");
	request.put ("source", nano::dev::genesis_key.pub.to_account ());
	request.put ("destination", nano::dev::genesis_key.pub.to_account ());
	request.put ("amount", "100");
	auto response (wait_response (system, rpc_ctx, request, 10s));
	ASSERT_EQ (std::error_code (nano::error_common::disabled_work_generation).message (), response.get<std::string> ("error"));
}

TEST (rpc, send_idempotent)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	(void)node->wallets.insert_adhoc (node->wallets.first_wallet_id (), nano::dev::genesis_key.prv);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	std::string wallet;
	node->wallets.first_wallet_id ().encode_hex (wallet);
	request.put ("wallet", wallet);
	request.put ("action", "send");
	request.put ("source", nano::dev::genesis_key.pub.to_account ());
	request.put ("destination", nano::account{}.to_account ());
	request.put ("amount", (nano::dev::constants.genesis_amount - (nano::dev::constants.genesis_amount / 4)).convert_to<std::string> ());
	request.put ("id", "123abc");
	auto response (wait_response (system, rpc_ctx, request));
	std::string block_text (response.get<std::string> ("block"));
	nano::block_hash block;
	ASSERT_FALSE (block.decode_hex (block_text));
	ASSERT_TRUE (node->block_or_pruned_exists (block));
	ASSERT_EQ (node->balance (nano::dev::genesis_key.pub), nano::dev::constants.genesis_amount / 4);
	auto response2 (wait_response (system, rpc_ctx, request));
	ASSERT_EQ ("", response2.get<std::string> ("error", ""));
	ASSERT_EQ (block_text, response2.get<std::string> ("block"));
	ASSERT_EQ (node->balance (nano::dev::genesis_key.pub), nano::dev::constants.genesis_amount / 4);
	request.erase ("id");
	request.put ("id", "456def");
	auto response3 (wait_response (system, rpc_ctx, request));
	ASSERT_EQ (std::error_code (nano::error_common::insufficient_balance).message (), response3.get<std::string> ("error"));
}

TEST (rpc, send_epoch_2)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);

	// Upgrade the genesis account to epoch 2
	std::shared_ptr<nano::block> epoch1, epoch2;
	ASSERT_TRUE (epoch1 = system.upgrade_genesis_epoch (*node, nano::epoch::epoch_1));
	ASSERT_TRUE (epoch2 = system.upgrade_genesis_epoch (*node, nano::epoch::epoch_2));

	(void)node->wallets.insert_adhoc (node->wallets.first_wallet_id (), nano::dev::genesis_key.prv, false);
	ASSERT_TIMELY (5s, nano::test::confirmed (*node, { epoch1, epoch2 }));

	auto target_difficulty = nano::dev::network_params.work.threshold (nano::work_version::work_1, nano::block_details (nano::epoch::epoch_2, true, false, false));
	ASSERT_LT (node->network_params.work.get_entry (), target_difficulty);
	auto min_difficulty = node->network_params.work.get_entry ();

	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	std::string wallet;
	node->wallets.first_wallet_id ().encode_hex (wallet);
	request.put ("wallet", wallet);
	request.put ("action", "send");
	request.put ("source", nano::dev::genesis_key.pub.to_account ());
	request.put ("destination", nano::keypair ().pub.to_account ());
	request.put ("amount", "1");

	// Test that the correct error is given if there is insufficient work
	auto insufficient = system.work_generate_limited (nano::dev::genesis->hash (), min_difficulty, target_difficulty);
	request.put ("work", nano::to_string_hex (insufficient));
	{
		auto response (wait_response (system, rpc_ctx, request));
		std::error_code ec (nano::error_common::invalid_work);
		ASSERT_EQ (1, response.count ("error"));
		ASSERT_EQ (response.get<std::string> ("error"), ec.message ());
	}
}

TEST (rpc, send_ipc_random_id)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node);
	std::atomic<bool> got_request{ false };
	rpc_ctx.node_rpc_config->set_request_callback ([&got_request] (boost::property_tree::ptree const & request_a) {
		EXPECT_TRUE (request_a.count ("id"));
		got_request = true;
	});
	boost::property_tree::ptree request;
	request.put ("action", "send");
	auto response (wait_response (system, rpc_ctx, request, 10s));
	ASSERT_EQ (1, response.count ("error"));
	ASSERT_EQ ("Unable to parse JSON", response.get<std::string> ("error"));
	ASSERT_TRUE (got_request);
}

TEST (rpc, stop)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	request.put ("action", "stop");
	auto response (wait_response (system, rpc_ctx, request));
}

TEST (rpc, wallet_add)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node);
	nano::keypair key1;
	std::string key_text;
	key1.prv.encode_hex (key_text);
	boost::property_tree::ptree request;
	std::string wallet;
	node->wallets.first_wallet_id ().encode_hex (wallet);
	request.put ("wallet", wallet);
	request.put ("action", "wallet_add");
	request.put ("key", key_text);
	auto response (wait_response (system, rpc_ctx, request));
	std::string account_text1 (response.get<std::string> ("account"));
	ASSERT_EQ (account_text1, key1.pub.to_account ());
	ASSERT_TRUE (node->wallets.exists (key1.pub));
}

TEST (rpc, wallet_password_valid)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	std::string wallet;
	node->wallets.first_wallet_id ().encode_hex (wallet);
	request.put ("wallet", wallet);
	request.put ("action", "password_valid");
	auto response (wait_response (system, rpc_ctx, request));
	std::string account_text1 (response.get<std::string> ("valid"));
	ASSERT_EQ (account_text1, "1");
}

TEST (rpc, wallet_password_change)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	auto wallet_id{ node->wallets.first_wallet_id () };
	std::string wallet;
	wallet_id.encode_hex (wallet);
	request.put ("wallet", wallet);
	request.put ("action", "password_change");
	request.put ("password", "test");
	auto response (wait_response (system, rpc_ctx, request));
	std::string account_text1 (response.get<std::string> ("changed"));
	ASSERT_EQ (account_text1, "1");
	bool valid = false;
	(void)node->wallets.valid_password (wallet_id, valid);
	ASSERT_TRUE (valid);
	ASSERT_EQ (nano::wallets_error::invalid_password, node->wallets.enter_password (wallet_id, ""));
	(void)node->wallets.valid_password (wallet_id, valid);
	ASSERT_FALSE (valid);
	ASSERT_EQ (nano::wallets_error::none, node->wallets.enter_password (wallet_id, "test"));
	(void)node->wallets.valid_password (wallet_id, valid);
	ASSERT_TRUE (valid);
}

TEST (rpc, wallet_password_enter)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto wallet_id = node->wallets.first_wallet_id ();

	auto const rpc_ctx = add_rpc (system, node);
	nano::raw_key password_l;
	password_l.clear ();
	system.deadline_set (10s);
	while (password_l == 0)
	{
		ASSERT_NO_ERROR (system.poll ());
		node->wallets.password (wallet_id, password_l);
	}
	boost::property_tree::ptree request;
	std::string wallet;
	node->wallets.first_wallet_id ().encode_hex (wallet);
	request.put ("wallet", wallet);
	request.put ("action", "password_enter");
	request.put ("password", "");
	auto response (wait_response (system, rpc_ctx, request));
	std::string account_text1 (response.get<std::string> ("valid"));
	ASSERT_EQ (account_text1, "1");
}

TEST (rpc, wallet_representative)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	std::string wallet;
	node->wallets.first_wallet_id ().encode_hex (wallet);
	request.put ("wallet", wallet);
	request.put ("action", "wallet_representative");
	auto response (wait_response (system, rpc_ctx, request));
	std::string account_text1 (response.get<std::string> ("representative"));
	ASSERT_EQ (account_text1, nano::dev::genesis_key.pub.to_account ());
}

TEST (rpc, wallet_representative_set)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	std::string wallet;
	node->wallets.first_wallet_id ().encode_hex (wallet);
	request.put ("wallet", wallet);
	nano::keypair key;
	request.put ("action", "wallet_representative_set");
	request.put ("representative", key.pub.to_account ());
	auto response (wait_response (system, rpc_ctx, request));
	auto wallet_id{ node->wallets.first_wallet_id () };
	nano::account representative;
	ASSERT_EQ (nano::wallets_error::none, node->wallets.get_representative (wallet_id, representative));
	ASSERT_EQ (key.pub, representative);
}

TEST (rpc, wallet_representative_set_force)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	(void)node->wallets.insert_adhoc (node->wallets.first_wallet_id (), nano::dev::genesis_key.prv);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	std::string wallet;
	auto wallet_id = node->wallets.first_wallet_id ();
	wallet_id.encode_hex (wallet);
	request.put ("wallet", wallet);
	nano::keypair key;
	request.put ("action", "wallet_representative_set");
	request.put ("representative", key.pub.to_account ());
	request.put ("update_existing_accounts", true);
	auto response (wait_response (system, rpc_ctx, request));
	{
		nano::account representative;
		(void)node->wallets.get_representative (wallet_id, representative);
		ASSERT_EQ (key.pub, representative);
	}
	nano::account representative{};
	while (representative != key.pub)
	{
		auto transaction (node->store.tx_begin_read ());
		auto info = node->ledger.any ().account_get (*transaction, nano::dev::genesis_key.pub);
		if (info)
		{
			representative = info->representative ();
		}
		ASSERT_NO_ERROR (system.poll ());
	}
}

TEST (rpc, account_list)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	nano::keypair key2;
	(void)node->wallets.insert_adhoc (node->wallets.first_wallet_id (), nano::dev::genesis_key.prv);
	(void)node->wallets.insert_adhoc (node->wallets.first_wallet_id (), key2.prv);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	std::string wallet;
	node->wallets.first_wallet_id ().encode_hex (wallet);
	request.put ("wallet", wallet);
	request.put ("action", "account_list");
	auto response (wait_response (system, rpc_ctx, request));
	auto & accounts_node (response.get_child ("accounts"));
	std::vector<nano::account> accounts;
	for (auto i (accounts_node.begin ()), j (accounts_node.end ()); i != j; ++i)
	{
		auto account (i->second.get<std::string> (""));
		nano::account number;
		ASSERT_FALSE (number.decode_account (account));
		accounts.push_back (number);
	}
	ASSERT_EQ (2, accounts.size ());
	for (auto i (accounts.begin ()), j (accounts.end ()); i != j; ++i)
	{
		ASSERT_TRUE (node->wallets.exists (*i));
	}
}

TEST (rpc, wallet_key_valid)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	(void)node->wallets.insert_adhoc (node->wallets.first_wallet_id (), nano::dev::genesis_key.prv);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	std::string wallet;
	node->wallets.first_wallet_id ().encode_hex (wallet);
	request.put ("wallet", wallet);
	request.put ("action", "wallet_key_valid");
	auto response (wait_response (system, rpc_ctx, request));
	std::string exists_text (response.get<std::string> ("valid"));
	ASSERT_EQ ("1", exists_text);
}

TEST (rpc, wallet_create)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	request.put ("action", "wallet_create");
	auto response (wait_response (system, rpc_ctx, request));
	std::string wallet_text (response.get<std::string> ("wallet"));
	nano::wallet_id wallet_id;
	ASSERT_FALSE (wallet_id.decode_hex (wallet_text));
	ASSERT_TRUE (node->wallets.wallet_exists (wallet_id));
}

TEST (rpc, wallet_create_seed)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	nano::raw_key seed;
	nano::random_pool::generate_block (seed.bytes.data (), seed.bytes.size ());
	auto prv = nano::deterministic_key (seed, 0);
	auto pub (nano::pub_key (prv));
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	request.put ("action", "wallet_create");
	request.put ("seed", seed.to_string ());
	auto response (wait_response (system, rpc_ctx, request, 10s));
	std::string wallet_text (response.get<std::string> ("wallet"));
	nano::wallet_id wallet_id;
	ASSERT_FALSE (wallet_id.decode_hex (wallet_text));
	ASSERT_TRUE (node->wallets.wallet_exists (wallet_id));
	nano::raw_key seed0;
	(void)node->wallets.get_seed (wallet_id, seed0);
	ASSERT_EQ (seed, seed0);
	auto account_text (response.get<std::string> ("last_restored_account"));
	nano::account account;
	ASSERT_FALSE (account.decode_account (account_text));
	std::vector<nano::account> accounts;
	(void)node->wallets.get_accounts (wallet_id, accounts);
	ASSERT_NE (std::find (accounts.begin (), accounts.end (), account), accounts.end ());
	ASSERT_EQ (pub, account);
	ASSERT_EQ ("1", response.get<std::string> ("restored_count"));
}

TEST (rpc, wallet_export)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	(void)node->wallets.insert_adhoc (node->wallets.first_wallet_id (), nano::dev::genesis_key.prv);
	auto const rpc_ctx = add_rpc (system, node);
	auto wallet_id{ node->wallets.first_wallet_id () };
	boost::property_tree::ptree request;
	request.put ("action", "wallet_export");
	request.put ("wallet", wallet_id.to_string ());
	auto response (wait_response (system, rpc_ctx, request));
	std::string wallet_json (response.get<std::string> ("json"));

	std::string expected_json;
	ASSERT_EQ (nano::wallets_error::none, node->wallets.serialize (wallet_id, expected_json));
	ASSERT_EQ (expected_json, wallet_json);
}

TEST (rpc, wallet_destroy)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	(void)node->wallets.insert_adhoc (node->wallets.first_wallet_id (), nano::dev::genesis_key.prv);
	auto const rpc_ctx = add_rpc (system, node);
	auto wallet_id (node->wallets.first_wallet_id ());
	boost::property_tree::ptree request;
	request.put ("action", "wallet_destroy");
	request.put ("wallet", wallet_id.to_string ());
	auto response (wait_response (system, rpc_ctx, request));
	ASSERT_FALSE (node->wallets.wallet_exists (wallet_id));
}

TEST (rpc, account_move)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto wallet_id (node->wallets.first_wallet_id ());
	(void)node->wallets.insert_adhoc (node->wallets.first_wallet_id (), nano::dev::genesis_key.prv);
	nano::keypair key;
	auto source_id = nano::random_wallet_id ();
	node->wallets.create (source_id);
	nano::account account;
	ASSERT_EQ (nano::wallets_error::none, node->wallets.insert_adhoc (source_id, key.prv, true, account));
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	request.put ("action", "account_move");
	request.put ("wallet", wallet_id.to_string ());
	request.put ("source", source_id.to_string ());
	boost::property_tree::ptree keys;
	boost::property_tree::ptree entry;
	entry.put ("", key.pub.to_account ());
	keys.push_back (std::make_pair ("", entry));
	request.add_child ("accounts", keys);
	auto response (wait_response (system, rpc_ctx, request));
	ASSERT_EQ ("1", response.get<std::string> ("moved"));
	ASSERT_TRUE (node->wallets.exists (key.pub));
	ASSERT_TRUE (node->wallets.exists (nano::dev::genesis_key.pub));
	std::vector<nano::account> accounts;
	ASSERT_EQ (nano::wallets_error::none, node->wallets.get_accounts (source_id, accounts));
	ASSERT_EQ (accounts.size (), 0);
}

TEST (rpc, block)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	request.put ("action", "block");
	request.put ("hash", node->latest (nano::dev::genesis_key.pub).to_string ());
	auto response (wait_response (system, rpc_ctx, request));
	auto contents (response.get<std::string> ("contents"));
	ASSERT_FALSE (contents.empty ());
	ASSERT_TRUE (response.get<bool> ("confirmed")); // Genesis block is confirmed by default
}

TEST (rpc, block_account)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	request.put ("action", "block_account");
	request.put ("hash", nano::dev::genesis->hash ().to_string ());
	auto response (wait_response (system, rpc_ctx, request));
	std::string account_text (response.get<std::string> ("account"));
	nano::account account;
	ASSERT_FALSE (account.decode_account (account_text));
}

TEST (rpc, chain)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto wallet_id = node->wallets.first_wallet_id ();
	(void)node->wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);
	nano::keypair key;
	auto genesis (node->latest (nano::dev::genesis_key.pub));
	ASSERT_FALSE (genesis.is_zero ());
	auto block (node->wallets.send_action (wallet_id, nano::dev::genesis_key.pub, key.pub, 1));
	ASSERT_NE (nullptr, block);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	request.put ("action", "chain");
	request.put ("block", block->hash ().to_string ());
	request.put ("count", std::to_string (std::numeric_limits<uint64_t>::max ()));
	auto response (wait_response (system, rpc_ctx, request));
	auto & blocks_node (response.get_child ("blocks"));
	std::vector<nano::block_hash> blocks;
	for (auto i (blocks_node.begin ()), n (blocks_node.end ()); i != n; ++i)
	{
		blocks.push_back (nano::block_hash (i->second.get<std::string> ("")));
	}
	ASSERT_EQ (2, blocks.size ());
	ASSERT_EQ (block->hash (), blocks[0]);
	ASSERT_EQ (genesis, blocks[1]);
}

TEST (rpc, chain_limit)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto wallet_id = node->wallets.first_wallet_id ();
	(void)node->wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);
	nano::keypair key;
	auto genesis (node->latest (nano::dev::genesis_key.pub));
	ASSERT_FALSE (genesis.is_zero ());
	auto block (node->wallets.send_action (wallet_id, nano::dev::genesis_key.pub, key.pub, 1));
	ASSERT_NE (nullptr, block);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	request.put ("action", "chain");
	request.put ("block", block->hash ().to_string ());
	request.put ("count", 1);
	auto response (wait_response (system, rpc_ctx, request));
	auto & blocks_node (response.get_child ("blocks"));
	std::vector<nano::block_hash> blocks;
	for (auto i (blocks_node.begin ()), n (blocks_node.end ()); i != n; ++i)
	{
		blocks.push_back (nano::block_hash (i->second.get<std::string> ("")));
	}
	ASSERT_EQ (1, blocks.size ());
	ASSERT_EQ (block->hash (), blocks[0]);
}

TEST (rpc, chain_offset)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto wallet_id = node->wallets.first_wallet_id ();
	(void)node->wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);
	nano::keypair key;
	auto genesis (node->latest (nano::dev::genesis_key.pub));
	ASSERT_FALSE (genesis.is_zero ());
	auto block (node->wallets.send_action (wallet_id, nano::dev::genesis_key.pub, key.pub, 1));
	ASSERT_NE (nullptr, block);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	request.put ("action", "chain");
	request.put ("block", block->hash ().to_string ());
	request.put ("count", std::to_string (std::numeric_limits<uint64_t>::max ()));
	request.put ("offset", 1);
	auto response (wait_response (system, rpc_ctx, request));
	auto & blocks_node (response.get_child ("blocks"));
	std::vector<nano::block_hash> blocks;
	for (auto i (blocks_node.begin ()), n (blocks_node.end ()); i != n; ++i)
	{
		blocks.push_back (nano::block_hash (i->second.get<std::string> ("")));
	}
	ASSERT_EQ (1, blocks.size ());
	ASSERT_EQ (genesis, blocks[0]);
}

TEST (rpc, frontier)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	std::unordered_map<nano::account, nano::block_hash> source;
	{
		auto transaction (node->store.tx_begin_write ());
		for (auto i (0); i < 1000; ++i)
		{
			nano::keypair key;
			nano::block_hash hash;
			nano::random_pool::generate_block (hash.bytes.data (), hash.bytes.size ());
			source[key.pub] = hash;
			node->store.account ().put (*transaction, key.pub, nano::account_info (hash, 0, 0, 0, 0, 0, nano::epoch::epoch_0));
		}
	}
	nano::keypair key;
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	request.put ("action", "frontiers");
	request.put ("account", nano::account{}.to_account ());
	request.put ("count", std::to_string (std::numeric_limits<uint64_t>::max ()));
	auto response (wait_response (system, rpc_ctx, request));
	auto & frontiers_node (response.get_child ("frontiers"));
	std::unordered_map<nano::account, nano::block_hash> frontiers;
	for (auto i (frontiers_node.begin ()), j (frontiers_node.end ()); i != j; ++i)
	{
		nano::account account;
		account.decode_account (i->first);
		nano::block_hash frontier;
		frontier.decode_hex (i->second.get<std::string> (""));
		frontiers[account] = frontier;
	}
	ASSERT_EQ (1, frontiers.erase (nano::dev::genesis_key.pub));
	ASSERT_EQ (source, frontiers);
}

TEST (rpc, frontier_limited)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	std::unordered_map<nano::account, nano::block_hash> source;
	{
		auto transaction (node->store.tx_begin_write ());
		for (auto i (0); i < 1000; ++i)
		{
			nano::keypair key;
			nano::block_hash hash;
			nano::random_pool::generate_block (hash.bytes.data (), hash.bytes.size ());
			source[key.pub] = hash;
			node->store.account ().put (*transaction, key.pub, nano::account_info (hash, 0, 0, 0, 0, 0, nano::epoch::epoch_0));
		}
	}
	nano::keypair key;
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	request.put ("action", "frontiers");
	request.put ("account", nano::account{}.to_account ());
	request.put ("count", std::to_string (100));
	auto response (wait_response (system, rpc_ctx, request));
	auto & frontiers_node (response.get_child ("frontiers"));
	ASSERT_EQ (100, frontiers_node.size ());
}

TEST (rpc, frontier_startpoint)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	std::unordered_map<nano::account, nano::block_hash> source;
	{
		auto transaction (node->store.tx_begin_write ());
		for (auto i (0); i < 1000; ++i)
		{
			nano::keypair key;
			nano::block_hash hash;
			nano::random_pool::generate_block (hash.bytes.data (), hash.bytes.size ());
			source[key.pub] = hash;
			node->store.account ().put (*transaction, key.pub, nano::account_info (hash, 0, 0, 0, 0, 0, nano::epoch::epoch_0));
		}
	}
	nano::keypair key;
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	request.put ("action", "frontiers");
	request.put ("account", source.begin ()->first.to_account ());
	request.put ("count", std::to_string (1));
	auto response (wait_response (system, rpc_ctx, request));
	auto & frontiers_node (response.get_child ("frontiers"));
	ASSERT_EQ (1, frontiers_node.size ());
	ASSERT_EQ (source.begin ()->first.to_account (), frontiers_node.begin ()->first);
}

TEST (rpc, history)
{
	nano::test::system system;
	auto node0 = add_ipc_enabled_node (system);
	auto wallet_id = node0->wallets.first_wallet_id ();
	(void)node0->wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);
	auto change (node0->wallets.change_action (wallet_id, nano::dev::genesis_key.pub, nano::dev::genesis_key.pub));
	ASSERT_NE (nullptr, change);
	auto send (node0->wallets.send_action (wallet_id, nano::dev::genesis_key.pub, nano::dev::genesis_key.pub, node0->config->receive_minimum.number ()));
	ASSERT_NE (nullptr, send);
	auto receive (node0->wallets.receive_action (wallet_id, send->hash (), nano::dev::genesis_key.pub, node0->config->receive_minimum.number (), send->destination ()));
	ASSERT_NE (nullptr, receive);
	nano::block_builder builder;
	auto usend = builder
				 .state ()
				 .account (nano::dev::genesis_key.pub)
				 .previous (node0->latest (nano::dev::genesis_key.pub))
				 .representative (nano::dev::genesis_key.pub)
				 .balance (nano::dev::constants.genesis_amount - nano::Gxrb_ratio)
				 .link (nano::dev::genesis_key.pub)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*node0->work_generate_blocking (node0->latest (nano::dev::genesis_key.pub)))
				 .build ();
	auto ureceive = builder
					.state ()
					.account (nano::dev::genesis_key.pub)
					.previous (usend->hash ())
					.representative (nano::dev::genesis_key.pub)
					.balance (nano::dev::constants.genesis_amount)
					.link (usend->hash ())
					.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
					.work (*node0->work_generate_blocking (usend->hash ()))
					.build ();
	auto uchange = builder
				   .state ()
				   .account (nano::dev::genesis_key.pub)
				   .previous (ureceive->hash ())
				   .representative (nano::keypair ().pub)
				   .balance (nano::dev::constants.genesis_amount)
				   .link (0)
				   .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				   .work (*node0->work_generate_blocking (ureceive->hash ()))
				   .build ();
	{
		auto transaction (node0->store.tx_begin_write ());
		ASSERT_EQ (nano::block_status::progress, node0->ledger.process (*transaction, usend));
		ASSERT_EQ (nano::block_status::progress, node0->ledger.process (*transaction, ureceive));
		ASSERT_EQ (nano::block_status::progress, node0->ledger.process (*transaction, uchange));
	}
	auto const rpc_ctx = add_rpc (system, node0);
	boost::property_tree::ptree request;
	request.put ("action", "history");
	request.put ("hash", uchange->hash ().to_string ());
	request.put ("count", 100);
	auto response (wait_response (system, rpc_ctx, request));
	std::vector<std::tuple<std::string, std::string, std::string, std::string>> history_l;
	auto & history_node (response.get_child ("history"));
	for (auto i (history_node.begin ()), n (history_node.end ()); i != n; ++i)
	{
		history_l.push_back (std::make_tuple (i->second.get<std::string> ("type"), i->second.get<std::string> ("account"), i->second.get<std::string> ("amount"), i->second.get<std::string> ("hash")));
	}
	ASSERT_EQ (5, history_l.size ());
	ASSERT_EQ ("receive", std::get<0> (history_l[0]));
	ASSERT_EQ (ureceive->hash ().to_string (), std::get<3> (history_l[0]));
	ASSERT_EQ (nano::dev::genesis_key.pub.to_account (), std::get<1> (history_l[0]));
	ASSERT_EQ (nano::Gxrb_ratio.convert_to<std::string> (), std::get<2> (history_l[0]));
	ASSERT_EQ (5, history_l.size ());
	ASSERT_EQ ("send", std::get<0> (history_l[1]));
	ASSERT_EQ (usend->hash ().to_string (), std::get<3> (history_l[1]));
	ASSERT_EQ (nano::dev::genesis_key.pub.to_account (), std::get<1> (history_l[1]));
	ASSERT_EQ (nano::Gxrb_ratio.convert_to<std::string> (), std::get<2> (history_l[1]));
	ASSERT_EQ ("receive", std::get<0> (history_l[2]));
	ASSERT_EQ (nano::dev::genesis_key.pub.to_account (), std::get<1> (history_l[2]));
	ASSERT_EQ (node0->config->receive_minimum.to_string_dec (), std::get<2> (history_l[2]));
	ASSERT_EQ (receive->hash ().to_string (), std::get<3> (history_l[2]));
	ASSERT_EQ ("send", std::get<0> (history_l[3]));
	ASSERT_EQ (nano::dev::genesis_key.pub.to_account (), std::get<1> (history_l[3]));
	ASSERT_EQ (node0->config->receive_minimum.to_string_dec (), std::get<2> (history_l[3]));
	ASSERT_EQ (send->hash ().to_string (), std::get<3> (history_l[3]));
	ASSERT_EQ ("receive", std::get<0> (history_l[4]));
	ASSERT_EQ (nano::dev::genesis_key.pub.to_account (), std::get<1> (history_l[4]));
	ASSERT_EQ (nano::dev::constants.genesis_amount.convert_to<std::string> (), std::get<2> (history_l[4]));
	ASSERT_EQ (nano::dev::genesis->hash ().to_string (), std::get<3> (history_l[4]));
}

TEST (rpc, account_history)
{
	nano::test::system system;
	auto node0 = add_ipc_enabled_node (system);
	auto wallet_id = node0->wallets.first_wallet_id ();
	(void)node0->wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);
	auto change (node0->wallets.change_action (wallet_id, nano::dev::genesis_key.pub, nano::dev::genesis_key.pub));
	ASSERT_NE (nullptr, change);
	auto send (node0->wallets.send_action (wallet_id, nano::dev::genesis_key.pub, nano::dev::genesis_key.pub, node0->config->receive_minimum.number ()));
	ASSERT_NE (nullptr, send);
	auto receive (node0->wallets.receive_action (wallet_id, send->hash (), nano::dev::genesis_key.pub, node0->config->receive_minimum.number (), send->destination ()));
	ASSERT_NE (nullptr, receive);
	nano::block_builder builder;
	auto usend = builder
				 .state ()
				 .account (nano::dev::genesis_key.pub)
				 .previous (node0->latest (nano::dev::genesis_key.pub))
				 .representative (nano::dev::genesis_key.pub)
				 .balance (nano::dev::constants.genesis_amount - nano::Gxrb_ratio)
				 .link (nano::dev::genesis_key.pub)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*node0->work_generate_blocking (node0->latest (nano::dev::genesis_key.pub)))
				 .build ();
	auto ureceive = builder
					.state ()
					.account (nano::dev::genesis_key.pub)
					.previous (usend->hash ())
					.representative (nano::dev::genesis_key.pub)
					.balance (nano::dev::constants.genesis_amount)
					.link (usend->hash ())
					.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
					.work (*node0->work_generate_blocking (usend->hash ()))
					.build ();
	auto uchange = builder
				   .state ()
				   .account (nano::dev::genesis_key.pub)
				   .previous (ureceive->hash ())
				   .representative (nano::keypair ().pub)
				   .balance (nano::dev::constants.genesis_amount)
				   .link (0)
				   .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				   .work (*node0->work_generate_blocking (ureceive->hash ()))
				   .build ();
	{
		auto transaction (node0->store.tx_begin_write ());
		ASSERT_EQ (nano::block_status::progress, node0->ledger.process (*transaction, usend));
		ASSERT_EQ (nano::block_status::progress, node0->ledger.process (*transaction, ureceive));
		ASSERT_EQ (nano::block_status::progress, node0->ledger.process (*transaction, uchange));
	}
	auto const rpc_ctx = add_rpc (system, node0);
	{
		boost::property_tree::ptree request;
		request.put ("action", "account_history");
		request.put ("account", nano::dev::genesis_key.pub.to_account ());
		request.put ("count", 100);
		auto response (wait_response (system, rpc_ctx, request, 10s));
		std::vector<std::tuple<std::string, std::string, std::string, std::string, std::string, bool>> history_l;
		auto & history_node (response.get_child ("history"));
		for (auto i (history_node.begin ()), n (history_node.end ()); i != n; ++i)
		{
			history_l.push_back (std::make_tuple (i->second.get<std::string> ("type"), i->second.get<std::string> ("account"), i->second.get<std::string> ("amount"), i->second.get<std::string> ("hash"), i->second.get<std::string> ("height"), i->second.get<bool> ("confirmed")));
		}

		ASSERT_EQ (5, history_l.size ());
		ASSERT_EQ ("receive", std::get<0> (history_l[0]));
		ASSERT_EQ (ureceive->hash ().to_string (), std::get<3> (history_l[0]));
		ASSERT_EQ (nano::dev::genesis_key.pub.to_account (), std::get<1> (history_l[0]));
		ASSERT_EQ (nano::Gxrb_ratio.convert_to<std::string> (), std::get<2> (history_l[0]));
		ASSERT_EQ ("6", std::get<4> (history_l[0])); // change block (height 7) is skipped by account_history since "raw" is not set
		ASSERT_FALSE (std::get<5> (history_l[0]));
		ASSERT_EQ ("send", std::get<0> (history_l[1]));
		ASSERT_EQ (usend->hash ().to_string (), std::get<3> (history_l[1]));
		ASSERT_EQ (nano::dev::genesis_key.pub.to_account (), std::get<1> (history_l[1]));
		ASSERT_EQ (nano::Gxrb_ratio.convert_to<std::string> (), std::get<2> (history_l[1]));
		ASSERT_EQ ("5", std::get<4> (history_l[1]));
		ASSERT_FALSE (std::get<5> (history_l[1]));
		ASSERT_EQ ("receive", std::get<0> (history_l[2]));
		ASSERT_EQ (nano::dev::genesis_key.pub.to_account (), std::get<1> (history_l[2]));
		ASSERT_EQ (node0->config->receive_minimum.to_string_dec (), std::get<2> (history_l[2]));
		ASSERT_EQ (receive->hash ().to_string (), std::get<3> (history_l[2]));
		ASSERT_EQ ("4", std::get<4> (history_l[2]));
		ASSERT_FALSE (std::get<5> (history_l[2]));
		ASSERT_EQ ("send", std::get<0> (history_l[3]));
		ASSERT_EQ (nano::dev::genesis_key.pub.to_account (), std::get<1> (history_l[3]));
		ASSERT_EQ (node0->config->receive_minimum.to_string_dec (), std::get<2> (history_l[3]));
		ASSERT_EQ (send->hash ().to_string (), std::get<3> (history_l[3]));
		ASSERT_EQ ("3", std::get<4> (history_l[3]));
		ASSERT_FALSE (std::get<5> (history_l[3]));
		ASSERT_EQ ("receive", std::get<0> (history_l[4]));
		ASSERT_EQ (nano::dev::genesis_key.pub.to_account (), std::get<1> (history_l[4]));
		ASSERT_EQ (nano::dev::constants.genesis_amount.convert_to<std::string> (), std::get<2> (history_l[4]));
		ASSERT_EQ (nano::dev::genesis->hash ().to_string (), std::get<3> (history_l[4]));
		ASSERT_EQ ("1", std::get<4> (history_l[4])); // change block (height 2) is skipped
		ASSERT_TRUE (std::get<5> (history_l[4]));
	}
	// Test count and reverse
	{
		boost::property_tree::ptree request;
		request.put ("action", "account_history");
		request.put ("account", nano::dev::genesis_key.pub.to_account ());
		request.put ("reverse", true);
		request.put ("count", 1);
		auto response (wait_response (system, rpc_ctx, request, 10s));
		auto & history_node (response.get_child ("history"));
		ASSERT_EQ (1, history_node.size ());
		ASSERT_EQ ("1", history_node.begin ()->second.get<std::string> ("height"));
		ASSERT_EQ (change->hash ().to_string (), response.get<std::string> ("next"));
	}

	// Test filtering
	nano::public_key account2;
	(void)node0->wallets.deterministic_insert (wallet_id, true, account2);
	auto send2 (node0->wallets.send_action (wallet_id, nano::dev::genesis_key.pub, account2, node0->config->receive_minimum.number ()));
	ASSERT_NE (nullptr, send2);
	auto receive2 (node0->wallets.receive_action (wallet_id, send2->hash (), account2, node0->config->receive_minimum.number (), send2->destination ()));
	// Test filter for send state blocks
	ASSERT_NE (nullptr, receive2);
	{
		boost::property_tree::ptree request;
		request.put ("action", "account_history");
		request.put ("account", nano::dev::genesis_key.pub.to_account ());
		boost::property_tree::ptree other_account;
		other_account.put ("", account2.to_account ());
		boost::property_tree::ptree filtered_accounts;
		filtered_accounts.push_back (std::make_pair ("", other_account));
		request.add_child ("account_filter", filtered_accounts);
		request.put ("count", 100);
		auto response (wait_response (system, rpc_ctx, request));
		auto history_node (response.get_child ("history"));
		ASSERT_EQ (history_node.size (), 2);
	}
	// Test filter for receive state blocks
	{
		boost::property_tree::ptree request;
		request.put ("action", "account_history");
		request.put ("account", account2.to_account ());
		boost::property_tree::ptree other_account;
		other_account.put ("", nano::dev::genesis_key.pub.to_account ());
		boost::property_tree::ptree filtered_accounts;
		filtered_accounts.push_back (std::make_pair ("", other_account));
		request.add_child ("account_filter", filtered_accounts);
		request.put ("count", 100);
		auto response (wait_response (system, rpc_ctx, request));
		auto history_node (response.get_child ("history"));
		ASSERT_EQ (history_node.size (), 1);
	}
}

TEST (rpc, history_count)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto wallet_id = node->wallets.first_wallet_id ();
	(void)node->wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);
	auto change (node->wallets.change_action (wallet_id, nano::dev::genesis_key.pub, nano::dev::genesis_key.pub));
	ASSERT_NE (nullptr, change);
	auto send (node->wallets.send_action (wallet_id, nano::dev::genesis_key.pub, nano::dev::genesis_key.pub, node->config->receive_minimum.number ()));
	ASSERT_NE (nullptr, send);
	auto receive (node->wallets.receive_action (wallet_id, send->hash (), nano::dev::genesis_key.pub, node->config->receive_minimum.number (), send->destination ()));
	ASSERT_NE (nullptr, receive);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	request.put ("action", "history");
	request.put ("hash", receive->hash ().to_string ());
	request.put ("count", 1);
	auto response (wait_response (system, rpc_ctx, request));
	auto & history_node (response.get_child ("history"));
	ASSERT_EQ (1, history_node.size ());
}

TEST (rpc, history_pruning)
{
	nano::test::system system;
	nano::node_config node_config = system.default_config ();
	node_config.enable_voting = false; // Remove after allowing pruned voting
	nano::node_flags node_flags;
	node_flags.set_enable_pruning (true);
	auto node0 = add_ipc_enabled_node (system, node_config, node_flags);
	auto wallet_id = node0->wallets.first_wallet_id ();
	std::vector<std::shared_ptr<nano::block>> blocks;

	nano::block_builder builder;

	// noop change block
	auto change = builder
				  .change ()
				  .previous (nano::dev::genesis->hash ())
				  .representative (nano::dev::genesis_key.pub)
				  .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				  .work (*node0->work.generate (nano::dev::genesis->hash ()))
				  .build ();
	blocks.push_back (change);

	// legacy send to itself
	auto send = builder
				.send ()
				.previous (change->hash ())
				.destination (nano::dev::genesis_key.pub)
				.balance (nano::dev::constants.genesis_amount - node0->config->receive_minimum.number ())
				.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				.work (*node0->work.generate (change->hash ()))
				.build ();
	blocks.push_back (send);

	// legacy receive the legacy self send
	auto receive = builder
				   .receive ()
				   .previous (send->hash ())
				   .source (send->hash ())
				   .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				   .work (*node0->work.generate (send->hash ()))
				   .build ();
	blocks.push_back (receive);

	// non legacy self send
	auto usend = builder
				 .state ()
				 .account (nano::dev::genesis_key.pub)
				 .previous (receive->hash ())
				 .representative (nano::dev::genesis_key.pub)
				 .balance (nano::dev::constants.genesis_amount - nano::Gxrb_ratio)
				 .link (nano::dev::genesis_key.pub)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*node0->work_generate_blocking (receive->hash ()))
				 .build ();
	blocks.push_back (usend);

	// non legacy receive of the non legacy self send
	auto ureceive = builder
					.state ()
					.account (nano::dev::genesis_key.pub)
					.previous (usend->hash ())
					.representative (nano::dev::genesis_key.pub)
					.balance (nano::dev::constants.genesis_amount)
					.link (usend->hash ())
					.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
					.work (*node0->work_generate_blocking (usend->hash ()))
					.build ();
	blocks.push_back (ureceive);

	// change genesis to a random rep
	auto uchange = builder
				   .state ()
				   .account (nano::dev::genesis_key.pub)
				   .previous (ureceive->hash ())
				   .representative (nano::keypair ().pub)
				   .balance (nano::dev::constants.genesis_amount)
				   .link (0)
				   .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				   .work (*node0->work_generate_blocking (ureceive->hash ()))
				   .build ();
	blocks.push_back (uchange);

	nano::test::process_live (*node0, blocks);
	ASSERT_TIMELY (5s, nano::test::exists (*node0, blocks));
	(void)node0->wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);

	nano::test::confirm (node0->ledger, blocks);

	// Prune block "change"
	{
		auto transaction (node0->store.tx_begin_write ());
		ASSERT_EQ (1, node0->ledger.pruning_action (*transaction, change->hash (), 1));
	}

	auto const rpc_ctx = add_rpc (system, node0);
	boost::property_tree::ptree request;
	request.put ("action", "history");
	request.put ("hash", send->hash ().to_string ());
	request.put ("count", 100);
	auto response = wait_response (system, rpc_ctx, request);
	auto history_node = response.get_child ("history");
	ASSERT_EQ (history_node.size (), 1);
	auto entry = (*history_node.begin ()).second;
	ASSERT_EQ ("send", entry.get<std::string> ("type"));
	ASSERT_EQ (nano::dev::genesis_key.pub.to_account (), entry.get<std::string> ("account", "N/A"));
	ASSERT_EQ ("N/A", entry.get<std::string> ("amount", "N/A"));
	ASSERT_EQ (send->hash ().to_string (), entry.get<std::string> ("hash"));

	// Prune block "send"
	{
		auto transaction (node0->store.tx_begin_write ());
		ASSERT_EQ (1, node0->ledger.pruning_action (*transaction, send->hash (), 1));
	}

	boost::property_tree::ptree request2;
	request2.put ("action", "history");
	request2.put ("hash", receive->hash ().to_string ());
	request2.put ("count", 100);
	response = wait_response (system, rpc_ctx, request2);
	history_node = response.get_child ("history");
	ASSERT_EQ (history_node.size (), 1);
	entry = (*history_node.begin ()).second;
	ASSERT_EQ ("receive", entry.get<std::string> ("type"));
	ASSERT_EQ ("N/A", entry.get<std::string> ("account", "N/A"));
	ASSERT_EQ ("N/A", entry.get<std::string> ("amount", "N/A"));
	ASSERT_EQ (receive->hash ().to_string (), entry.get<std::string> ("hash"));

	// Prune block "receive"
	{
		auto transaction (node0->store.tx_begin_write ());
		ASSERT_EQ (1, node0->ledger.pruning_action (*transaction, receive->hash (), 1));
	}

	boost::property_tree::ptree request3;
	request3.put ("action", "history");
	request3.put ("hash", uchange->hash ().to_string ());
	request3.put ("count", 100);
	response = wait_response (system, rpc_ctx, request3);
	history_node = response.get_child ("history");
	ASSERT_EQ (history_node.size (), 2);

	// first array element
	entry = (*history_node.begin ()).second;
	ASSERT_EQ ("receive", entry.get<std::string> ("type"));
	ASSERT_EQ (ureceive->hash ().to_string (), entry.get<std::string> ("hash"));
	ASSERT_EQ (nano::dev::genesis_key.pub.to_account (), entry.get<std::string> ("account", "N/A"));
	ASSERT_EQ (nano::Gxrb_ratio.convert_to<std::string> (), entry.get<std::string> ("amount", "N/A"));

	// second array element
	entry = (*(++history_node.begin ())).second;
	ASSERT_EQ ("unknown", entry.get<std::string> ("type"));
	ASSERT_EQ ("N/A", entry.get<std::string> ("account", "N/A"));
	ASSERT_EQ ("N/A", entry.get<std::string> ("amount", "N/A"));
	ASSERT_EQ (usend->hash ().to_string (), entry.get<std::string> ("hash"));
}

TEST (rpc, account_history_state_open)
{
	nano::test::system system;
	nano::keypair key;
	auto node0 = add_ipc_enabled_node (system);
	auto blocks = nano::test::setup_new_account (system, *node0, 1, nano::dev::genesis_key, key, key.pub, true);
	auto const rpc_ctx = add_rpc (system, node0);
	boost::property_tree::ptree request;
	request.put ("action", "account_history");
	request.put ("account", key.pub.to_account ());
	request.put ("count", 1);
	auto response (wait_response (system, rpc_ctx, request, 10s));
	auto & history_node (response.get_child ("history"));
	ASSERT_EQ (1, history_node.size ());
	auto history0 = *history_node.begin ();
	ASSERT_EQ ("1", history0.second.get<std::string> ("height"));
	ASSERT_EQ ("receive", history0.second.get<std::string> ("type"));
	ASSERT_EQ (blocks.second->hash ().to_string (), history0.second.get<std::string> ("hash"));
}

TEST (rpc, process_block)
{
	nano::test::system system;
	auto node1 = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node1);
	nano::keypair key;
	auto latest (node1->latest (nano::dev::genesis_key.pub));
	nano::block_builder builder;
	auto send = builder
				.send ()
				.previous (latest)
				.destination (key.pub)
				.balance (100)
				.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				.work (*node1->work_generate_blocking (latest))
				.build ();
	boost::property_tree::ptree request;
	request.put ("action", "process");
	std::string json;
	send->serialize_json (json);
	request.put ("block", json);
	{
		auto response (wait_response (system, rpc_ctx, request));
		ASSERT_TIMELY_EQ (10s, node1->latest (nano::dev::genesis_key.pub), send->hash ());
		std::string send_hash (response.get<std::string> ("hash"));
		ASSERT_EQ (send->hash ().to_string (), send_hash);
	}
	request.put ("json_block", true);
	{
		auto response (wait_response (system, rpc_ctx, request));
		std::error_code ec (nano::error_blocks::invalid_block);
		ASSERT_EQ (ec.message (), response.get<std::string> ("error"));
	}
}

TEST (rpc, process_json_block)
{
	nano::test::system system;
	auto node1 = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node1);
	nano::keypair key;
	auto latest (node1->latest (nano::dev::genesis_key.pub));
	nano::block_builder builder;
	auto send = builder
				.send ()
				.previous (latest)
				.destination (key.pub)
				.balance (100)
				.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				.work (*node1->work_generate_blocking (latest))
				.build ();
	boost::property_tree::ptree request;
	request.put ("action", "process");
	boost::property_tree::ptree block_node;
	send->serialize_json (block_node);
	request.add_child ("block", block_node);
	{
		auto response (wait_response (system, rpc_ctx, request));
		std::error_code ec (nano::error_blocks::invalid_block);
		ASSERT_EQ (ec.message (), response.get<std::string> ("error"));
	}
	request.put ("json_block", true);
	{
		auto response (wait_response (system, rpc_ctx, request));
		ASSERT_TIMELY_EQ (10s, node1->latest (nano::dev::genesis_key.pub), send->hash ());
		std::string send_hash (response.get<std::string> ("hash"));
		ASSERT_EQ (send->hash ().to_string (), send_hash);
	}
}

TEST (rpc, process_block_async)
{
	nano::test::system system;
	auto node1 = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node1);
	nano::keypair key;
	auto latest (node1->latest (nano::dev::genesis_key.pub));
	nano::block_builder builder;
	auto send = builder
				.send ()
				.previous (latest)
				.destination (key.pub)
				.balance (100)
				.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				.work (*node1->work_generate_blocking (latest))
				.build ();
	boost::property_tree::ptree request;
	request.put ("action", "process");
	request.put ("async", "true");
	std::string json;
	send->serialize_json (json);
	request.put ("block", json);
	request.put ("json_block", true);
	{
		auto response (wait_response (system, rpc_ctx, request));
		std::error_code ec (nano::error_blocks::invalid_block);
		ASSERT_EQ (ec.message (), response.get<std::string> ("error"));
	}
	request.put ("json_block", false);
	{
		auto response (wait_response (system, rpc_ctx, request));
		std::error_code ec (nano::error_common::is_not_state_block);
		ASSERT_EQ (ec.message (), response.get<std::string> ("error"));
	}

	auto state_send = builder
					  .state ()
					  .account (nano::dev::genesis_key.pub)
					  .previous (latest)
					  .representative (nano::dev::genesis_key.pub)
					  .balance (nano::dev::constants.genesis_amount - 100)
					  .link (nano::dev::genesis_key.pub)
					  .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
					  .work (*system.work.generate (latest))
					  .build ();
	std::string json1;
	state_send->serialize_json (json1);
	request.put ("block", json1);
	{
		auto response (wait_response (system, rpc_ctx, request));
		ASSERT_EQ ("1", response.get<std::string> ("started"));
		ASSERT_TIMELY_EQ (10s, node1->latest (nano::dev::genesis_key.pub), state_send->hash ());
	}
}

TEST (rpc, process_block_no_work)
{
	nano::test::system system;
	auto node1 = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node1);
	nano::keypair key;
	auto latest (node1->latest (nano::dev::genesis_key.pub));
	nano::block_builder builder;
	auto send = builder
				.send ()
				.previous (latest)
				.destination (key.pub)
				.balance (100)
				.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				.work (*node1->work_generate_blocking (latest))
				.build ();
	send->block_work_set (0);
	boost::property_tree::ptree request;
	request.put ("action", "process");
	std::string json;
	send->serialize_json (json);
	request.put ("block", json);
	auto response (wait_response (system, rpc_ctx, request));
	ASSERT_FALSE (response.get<std::string> ("error", "").empty ());
}

TEST (rpc, process_republish)
{
	nano::test::system system (2);
	auto & node1 (*system.nodes[0]);
	auto & node2 (*system.nodes[1]);
	auto node3 = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node3);
	nano::keypair key;
	auto latest (node1.latest (nano::dev::genesis_key.pub));
	nano::block_builder builder;
	auto send = builder
				.send ()
				.previous (latest)
				.destination (key.pub)
				.balance (100)
				.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				.work (*node3->work_generate_blocking (latest))
				.build ();
	boost::property_tree::ptree request;
	request.put ("action", "process");
	std::string json;
	send->serialize_json (json);
	request.put ("block", json);
	auto response (wait_response (system, rpc_ctx, request));
	ASSERT_TIMELY_EQ (10s, node2.latest (nano::dev::genesis_key.pub), send->hash ());
}

TEST (rpc, process_subtype_send)
{
	nano::test::system system;
	auto node1 = add_ipc_enabled_node (system);
	system.add_node ();
	auto const rpc_ctx = add_rpc (system, node1);
	nano::keypair key;
	auto latest (node1->latest (nano::dev::genesis_key.pub));
	nano::block_builder builder;
	auto send = builder
				.state ()
				.account (nano::dev::genesis_key.pub)
				.previous (latest)
				.representative (nano::dev::genesis_key.pub)
				.balance (nano::dev::constants.genesis_amount - nano::Gxrb_ratio)
				.link (key.pub)
				.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				.work (*node1->work_generate_blocking (latest))
				.build ();
	boost::property_tree::ptree request;
	request.put ("action", "process");
	std::string json;
	send->serialize_json (json);
	request.put ("block", json);
	request.put ("subtype", "receive");
	auto response (wait_response (system, rpc_ctx, request));
	std::error_code ec (nano::error_rpc::invalid_subtype_balance);
	ASSERT_EQ (response.get<std::string> ("error"), ec.message ());
	request.put ("subtype", "change");
	auto response2 (wait_response (system, rpc_ctx, request));
	ASSERT_EQ (response2.get<std::string> ("error"), ec.message ());
	request.put ("subtype", "send");
	auto response3 (wait_response (system, rpc_ctx, request));
	ASSERT_EQ (send->hash ().to_string (), response3.get<std::string> ("hash"));
	ASSERT_TIMELY_EQ (10s, system.nodes[1]->latest (nano::dev::genesis_key.pub), send->hash ());
}

TEST (rpc, process_subtype_open)
{
	nano::test::system system;
	auto node1 = add_ipc_enabled_node (system);
	auto & node2 = *system.add_node ();
	nano::keypair key;
	auto latest (node1->latest (nano::dev::genesis_key.pub));
	nano::block_builder builder;
	auto send = builder
				.state ()
				.account (nano::dev::genesis_key.pub)
				.previous (latest)
				.representative (nano::dev::genesis_key.pub)
				.balance (nano::dev::constants.genesis_amount - nano::Gxrb_ratio)
				.link (key.pub)
				.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				.work (*node1->work_generate_blocking (latest))
				.build ();
	ASSERT_EQ (nano::block_status::progress, node1->process (send));
	ASSERT_EQ (nano::block_status::progress, node2.process (send));
	auto const rpc_ctx = add_rpc (system, node1);
	node1->scheduler.manual.push (send);
	auto open = builder
				.state ()
				.account (key.pub)
				.previous (0)
				.representative (key.pub)
				.balance (nano::Gxrb_ratio)
				.link (send->hash ())
				.sign (key.prv, key.pub)
				.work (*node1->work_generate_blocking (key.pub))
				.build ();
	boost::property_tree::ptree request;
	request.put ("action", "process");
	std::string json;
	open->serialize_json (json);
	request.put ("block", json);
	request.put ("subtype", "send");
	auto response (wait_response (system, rpc_ctx, request));
	std::error_code ec (nano::error_rpc::invalid_subtype_balance);
	ASSERT_EQ (response.get<std::string> ("error"), ec.message ());
	request.put ("subtype", "epoch");
	auto response2 (wait_response (system, rpc_ctx, request));
	ASSERT_EQ (response2.get<std::string> ("error"), ec.message ());
	request.put ("subtype", "open");
	auto response3 (wait_response (system, rpc_ctx, request));
	ASSERT_EQ (open->hash ().to_string (), response3.get<std::string> ("hash"));
	ASSERT_TIMELY_EQ (10s, node2.latest (key.pub), open->hash ());
}

TEST (rpc, process_subtype_receive)
{
	nano::test::system system;
	auto node1 = add_ipc_enabled_node (system);
	auto & node2 = *system.add_node ();
	auto latest (node1->latest (nano::dev::genesis_key.pub));
	nano::block_builder builder;
	auto send = builder
				.state ()
				.account (nano::dev::genesis_key.pub)
				.previous (latest)
				.representative (nano::dev::genesis_key.pub)
				.balance (nano::dev::constants.genesis_amount - nano::Gxrb_ratio)
				.link (nano::dev::genesis_key.pub)
				.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				.work (*node1->work_generate_blocking (latest))
				.build ();
	ASSERT_EQ (nano::block_status::progress, node1->process (send));
	ASSERT_EQ (nano::block_status::progress, node2.process (send));
	auto const rpc_ctx = add_rpc (system, node1);
	node1->scheduler.manual.push (send);
	auto receive = builder
				   .state ()
				   .account (nano::dev::genesis_key.pub)
				   .previous (send->hash ())
				   .representative (nano::dev::genesis_key.pub)
				   .balance (nano::dev::constants.genesis_amount)
				   .link (send->hash ())
				   .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				   .work (*node1->work_generate_blocking (send->hash ()))
				   .build ();
	boost::property_tree::ptree request;
	request.put ("action", "process");
	std::string json;
	receive->serialize_json (json);
	request.put ("block", json);
	request.put ("subtype", "send");
	auto response (wait_response (system, rpc_ctx, request));
	std::error_code ec (nano::error_rpc::invalid_subtype_balance);
	ASSERT_EQ (response.get<std::string> ("error"), ec.message ());
	request.put ("subtype", "open");
	auto response2 (wait_response (system, rpc_ctx, request));
	ec = nano::error_rpc::invalid_subtype_previous;
	ASSERT_EQ (response2.get<std::string> ("error"), ec.message ());
	request.put ("subtype", "receive");
	auto response3 (wait_response (system, rpc_ctx, request));
	ASSERT_EQ (receive->hash ().to_string (), response3.get<std::string> ("hash"));
	ASSERT_TIMELY_EQ (10s, node2.latest (nano::dev::genesis_key.pub), receive->hash ());
}

TEST (rpc, process_ledger_insufficient_work)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node);
	ASSERT_LT (node->network_params.work.get_entry (), node->network_params.work.get_epoch_1 ());
	auto latest (node->latest (nano::dev::genesis_key.pub));
	auto min_difficulty = node->network_params.work.get_entry ();
	auto max_difficulty = node->network_params.work.get_epoch_1 ();
	nano::block_builder builder;
	auto send = builder
				.state ()
				.account (nano::dev::genesis_key.pub)
				.previous (latest)
				.representative (nano::dev::genesis_key.pub)
				.balance (nano::dev::constants.genesis_amount - nano::Gxrb_ratio)
				.link (nano::dev::genesis_key.pub)
				.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				.work (system.work_generate_limited (latest, min_difficulty, max_difficulty))
				.build ();
	ASSERT_LT (nano::dev::network_params.work.difficulty (*send), max_difficulty);
	ASSERT_GE (nano::dev::network_params.work.difficulty (*send), min_difficulty);
	boost::property_tree::ptree request;
	request.put ("action", "process");
	std::string json;
	send->serialize_json (json);
	request.put ("block", json);
	request.put ("subtype", "send");
	auto response (wait_response (system, rpc_ctx, request));
	std::error_code ec (nano::error_process::insufficient_work);
	ASSERT_EQ (1, response.count ("error"));
	ASSERT_EQ (response.get<std::string> ("error"), ec.message ());
}

TEST (rpc, keepalive)
{
	nano::test::system system;
	auto node0 = add_ipc_enabled_node (system);
	auto node1 (std::make_shared<nano::node> (system.async_rt, system.get_available_port (), nano::unique_path (), system.work));
	node1->start ();
	system.nodes.push_back (node1);
	auto const rpc_ctx = add_rpc (system, node0);
	boost::property_tree::ptree request;
	request.put ("action", "keepalive");
	auto address (boost::str (boost::format ("%1%") % node1->network->endpoint ().address ()));
	auto port (boost::str (boost::format ("%1%") % node1->network->endpoint ().port ()));
	request.put ("address", address);
	request.put ("port", port);
	ASSERT_FALSE (node0->find_endpoint_for_node_id (node1->get_node_id ()).has_value ());
	ASSERT_EQ (0, node0->network->size ());
	auto response (wait_response (system, rpc_ctx, request));
	system.deadline_set (10s);
	while (node0->find_endpoint_for_node_id (node1->get_node_id ()).has_value () == false)
	{
		ASSERT_EQ (0, node0->network->size ());
		ASSERT_NO_ERROR (system.poll ());
	}
}

TEST (rpc, peers)
{
	nano::test::system system;
	// Add node2 first to avoid peers with ephemeral ports
	auto const node2 = system.add_node ();
	auto node = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	request.put ("action", "peers");
	auto response (wait_response (system, rpc_ctx, request));
	auto & peers_node (response.get_child ("peers"));
	ASSERT_EQ (1, peers_node.size ());

	auto peer = peers_node.begin ();
	ASSERT_EQ (peer->first, boost::lexical_cast<std::string> (node2->network->endpoint ()));
	ASSERT_EQ (std::to_string (node->network_params.network.protocol_version), peers_node.get<std::string> (peer->first));
	// The previous version of this test had an UDP connection to an arbitrary IP address, so it could check for two peers. This doesn't work with TCP.
}

TEST (rpc, peers_node_id)
{
	nano::test::system system;
	// Add node2 first to avoid peers with ephemeral ports
	auto const node2 = system.add_node ();
	auto node = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	request.put ("action", "peers");
	request.put ("peer_details", true);
	auto response (wait_response (system, rpc_ctx, request));
	auto & peers_node (response.get_child ("peers"));
	ASSERT_EQ (1, peers_node.size ());

	auto peer = peers_node.begin ();
	ASSERT_EQ (peer->first, boost::lexical_cast<std::string> (node2->network->endpoint ()));

	auto tree1 = peer->second;
	ASSERT_EQ (std::to_string (node->network_params.network.protocol_version), tree1.get<std::string> ("protocol_version"));
	ASSERT_EQ (node2->node_id.pub.to_node_id (), tree1.get<std::string> ("node_id"));
	// The previous version of this test had an UDP connection to an arbitrary IP address, so it could check for two peers. This doesn't work with TCP.
}

TEST (rpc, peers_peering_endpoint)
{
	nano::test::system system;
	// Add node first, so that node2 will connect to node from ephemeral port
	auto node = add_ipc_enabled_node (system);
	auto const node2 = system.add_node ();
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	request.put ("action", "peers");
	request.put ("peer_details", true);
	auto response (wait_response (system, rpc_ctx, request));
	auto & peers_node (response.get_child ("peers"));
	ASSERT_EQ (1, peers_node.size ());

	auto peer = peers_node.begin ();
	ASSERT_NE (peer->first, boost::lexical_cast<std::string> (node2->network->endpoint ()));
	ASSERT_EQ (peer->second.get<std::string> ("peering"), boost::lexical_cast<std::string> (node2->network->endpoint ()));
}

TEST (rpc, version)
{
	nano::test::system system;
	auto node1 = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node1);
	boost::property_tree::ptree request1;
	request1.put ("action", "version");
	test_response response1 (request1, rpc_ctx.rpc->listening_port (), system.async_rt.io_ctx);
	ASSERT_TIMELY (5s, response1.status != 0);
	ASSERT_EQ (200, response1.status);
	ASSERT_EQ ("1", response1.json.get<std::string> ("rpc_version"));
	{
		auto transaction (node1->store.tx_begin_read ());
		ASSERT_EQ (std::to_string (node1->store.version ().get (*transaction)), response1.json.get<std::string> ("store_version"));
	}
	ASSERT_EQ (std::to_string (node1->network_params.network.protocol_version), response1.json.get<std::string> ("protocol_version"));
	ASSERT_EQ (boost::str (boost::format ("RsNano %1%") % NANO_VERSION_STRING), response1.json.get<std::string> ("node_vendor"));
	ASSERT_EQ (node1->store.vendor_get (), response1.json.get<std::string> ("store_vendor"));
	auto network_label (node1->network_params.network.get_current_network_as_string ());
	ASSERT_EQ (network_label, response1.json.get<std::string> ("network"));
	auto genesis_open (node1->latest (nano::dev::genesis_key.pub));
	ASSERT_EQ (genesis_open.to_string (), response1.json.get<std::string> ("network_identifier"));
	ASSERT_EQ (BUILD_INFO, response1.json.get<std::string> ("build_info"));
	auto headers (response1.resp.base ());
	auto allow (headers.at ("Allow"));
	auto content_type (headers.at ("Content-Type"));
	auto access_control_allow_origin (headers.at ("Access-Control-Allow-Origin"));
	auto access_control_allow_methods (headers.at ("Access-Control-Allow-Methods"));
	auto access_control_allow_headers (headers.at ("Access-Control-Allow-Headers"));
	auto connection (headers.at ("Connection"));
	ASSERT_EQ ("POST, OPTIONS", allow);
	ASSERT_EQ ("application/json", content_type);
	ASSERT_EQ ("*", access_control_allow_origin);
	ASSERT_EQ (allow, access_control_allow_methods);
	ASSERT_EQ ("Accept, Accept-Language, Content-Language, Content-Type", access_control_allow_headers);
	ASSERT_EQ ("close", connection);
}

TEST (rpc, work_generate)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node);
	nano::block_hash hash (1);
	boost::property_tree::ptree request;
	request.put ("action", "work_generate");
	request.put ("hash", hash.to_string ());
	auto verify_response = [&node, &rpc_ctx, &system] (auto & request, auto & hash) {
		auto response (wait_response (system, rpc_ctx, request));
		ASSERT_EQ (hash.to_string (), response.template get<std::string> ("hash"));
		auto work_text (response.template get<std::string> ("work"));
		uint64_t work;
		ASSERT_FALSE (nano::from_string_hex (work_text, work));
		auto result_difficulty (nano::dev::network_params.work.difficulty (nano::work_version::work_1, hash, work));
		auto response_difficulty_text (response.template get<std::string> ("difficulty"));
		uint64_t response_difficulty;
		ASSERT_FALSE (nano::from_string_hex (response_difficulty_text, response_difficulty));
		ASSERT_EQ (result_difficulty, response_difficulty);
		auto multiplier = response.template get<double> ("multiplier");
		ASSERT_NEAR (nano::difficulty::to_multiplier (result_difficulty, node->default_difficulty (nano::work_version::work_1)), multiplier, 1e-6);
	};
	verify_response (request, hash);
	request.put ("use_peers", "true");
	verify_response (request, hash);
}

TEST (rpc, work_generate_difficulty)
{
	nano::test::system system;
	nano::node_config node_config = system.default_config ();
	node_config.max_work_generate_multiplier = 1000;
	auto node = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node);
	nano::block_hash hash (1);
	boost::property_tree::ptree request;
	request.put ("action", "work_generate");
	request.put ("hash", hash.to_string ());
	{
		uint64_t difficulty (0xfff0000000000000);
		request.put ("difficulty", nano::to_string_hex (difficulty));
		auto response (wait_response (system, rpc_ctx, request, 10s));
		auto work_text (response.get<std::string> ("work"));
		uint64_t work;
		ASSERT_FALSE (nano::from_string_hex (work_text, work));
		auto result_difficulty (nano::dev::network_params.work.difficulty (nano::work_version::work_1, hash, work));
		auto response_difficulty_text (response.get<std::string> ("difficulty"));
		uint64_t response_difficulty;
		ASSERT_FALSE (nano::from_string_hex (response_difficulty_text, response_difficulty));
		ASSERT_EQ (result_difficulty, response_difficulty);
		auto multiplier = response.get<double> ("multiplier");
		// Expected multiplier from base threshold, not from the given difficulty
		ASSERT_NEAR (nano::difficulty::to_multiplier (result_difficulty, node->default_difficulty (nano::work_version::work_1)), multiplier, 1e-10);
		ASSERT_GE (result_difficulty, difficulty);
	}
	{
		uint64_t difficulty (0xffff000000000000);
		request.put ("difficulty", nano::to_string_hex (difficulty));
		auto response (wait_response (system, rpc_ctx, request));
		auto work_text (response.get<std::string> ("work"));
		uint64_t work;
		ASSERT_FALSE (nano::from_string_hex (work_text, work));
		auto result_difficulty (nano::dev::network_params.work.difficulty (nano::work_version::work_1, hash, work));
		ASSERT_GE (result_difficulty, difficulty);
	}
	{
		uint64_t difficulty (node->max_work_generate_difficulty (nano::work_version::work_1) + 1);
		request.put ("difficulty", nano::to_string_hex (difficulty));
		auto response (wait_response (system, rpc_ctx, request));
		std::error_code ec (nano::error_rpc::difficulty_limit);
		ASSERT_EQ (response.get<std::string> ("error"), ec.message ());
	}
}

TEST (rpc, work_generate_multiplier)
{
	nano::test::system system;
	nano::node_config node_config = system.default_config ();
	node_config.max_work_generate_multiplier = 100;
	auto node = add_ipc_enabled_node (system, node_config);
	auto const rpc_ctx = add_rpc (system, node);
	nano::block_hash hash (1);
	boost::property_tree::ptree request;
	request.put ("action", "work_generate");
	request.put ("hash", hash.to_string ());
	{
		// When both difficulty and multiplier are given, should use multiplier
		// Give base difficulty and very high multiplier to test
		request.put ("difficulty", nano::to_string_hex (static_cast<uint64_t> (0xff00000000000000)));
		double multiplier{ 100.0 };
		request.put ("multiplier", multiplier);
		auto response (wait_response (system, rpc_ctx, request, 10s));
		auto work_text (response.get_optional<std::string> ("work"));
		ASSERT_TRUE (work_text.is_initialized ());
		uint64_t work;
		ASSERT_FALSE (nano::from_string_hex (*work_text, work));
		auto result_difficulty (nano::dev::network_params.work.difficulty (nano::work_version::work_1, hash, work));
		auto response_difficulty_text (response.get<std::string> ("difficulty"));
		uint64_t response_difficulty;
		ASSERT_FALSE (nano::from_string_hex (response_difficulty_text, response_difficulty));
		ASSERT_EQ (result_difficulty, response_difficulty);
		auto result_multiplier = response.get<double> ("multiplier");
		ASSERT_GE (result_multiplier, multiplier);
	}
	{
		request.put ("multiplier", -1.5);
		auto response (wait_response (system, rpc_ctx, request));
		std::error_code ec (nano::error_rpc::bad_multiplier_format);
		ASSERT_EQ (response.get<std::string> ("error"), ec.message ());
	}
	{
		double max_multiplier (nano::difficulty::to_multiplier (node->max_work_generate_difficulty (nano::work_version::work_1), node->default_difficulty (nano::work_version::work_1)));
		request.put ("multiplier", max_multiplier + 1);
		auto response (wait_response (system, rpc_ctx, request));
		std::error_code ec (nano::error_rpc::difficulty_limit);
		ASSERT_EQ (response.get<std::string> ("error"), ec.message ());
	}
}

TEST (rpc, work_generate_block_high)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node);
	nano::keypair key;
	nano::block_builder builder;
	auto block = builder
				 .state ()
				 .account (key.pub)
				 .previous (0)
				 .representative (nano::dev::genesis_key.pub)
				 .balance (nano::Gxrb_ratio)
				 .link (123)
				 .sign (key.prv, key.pub)
				 .work (*node->work_generate_blocking (key.pub))
				 .build ();
	nano::block_hash hash (block->root ().as_block_hash ());
	auto block_difficulty (nano::dev::network_params.work.difficulty (nano::work_version::work_1, hash, block->block_work ()));
	boost::property_tree::ptree request;
	request.put ("action", "work_generate");
	request.put ("hash", hash.to_string ());
	request.put ("json_block", "true");
	boost::property_tree::ptree json;
	block->serialize_json (json);
	request.add_child ("block", json);
	{
		auto response (wait_response (system, rpc_ctx, request));
		ASSERT_EQ (1, response.count ("error"));
		ASSERT_EQ (std::error_code (nano::error_rpc::block_work_enough).message (), response.get<std::string> ("error"));
	}
}

TEST (rpc, work_generate_block_low)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node);
	nano::keypair key;
	nano::block_builder builder;
	auto block = builder
				 .state ()
				 .account (key.pub)
				 .previous (0)
				 .representative (nano::dev::genesis_key.pub)
				 .balance (nano::Gxrb_ratio)
				 .link (123)
				 .sign (key.prv, key.pub)
				 .work (0)
				 .build ();
	auto threshold (node->default_difficulty (block->work_version ()));
	block->block_work_set (system.work_generate_limited (block->root ().as_block_hash (), threshold, nano::difficulty::from_multiplier (node->config->max_work_generate_multiplier / 10, threshold)));
	nano::block_hash hash (block->root ().as_block_hash ());
	auto block_difficulty (nano::dev::network_params.work.difficulty (*block));
	boost::property_tree::ptree request;
	request.put ("action", "work_generate");
	request.put ("hash", hash.to_string ());
	request.put ("difficulty", nano::to_string_hex (block_difficulty + 1));
	request.put ("json_block", "false");
	std::string json;
	block->serialize_json (json);
	request.put ("block", json);
	{
		auto response (wait_response (system, rpc_ctx, request, 10s));
		auto work_text (response.get_optional<std::string> ("work"));
		ASSERT_TRUE (work_text.is_initialized ());
		uint64_t work;
		ASSERT_FALSE (nano::from_string_hex (*work_text, work));
		ASSERT_NE (block->block_work (), work);
		auto result_difficulty (nano::dev::network_params.work.difficulty (nano::work_version::work_1, hash, work));
		auto response_difficulty_text (response.get<std::string> ("difficulty"));
		uint64_t response_difficulty;
		ASSERT_FALSE (nano::from_string_hex (response_difficulty_text, response_difficulty));
		ASSERT_EQ (result_difficulty, response_difficulty);
		ASSERT_LT (block_difficulty, result_difficulty);
	}
}

TEST (rpc, work_generate_block_root_mismatch)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node);
	nano::keypair key;
	nano::block_builder builder;
	auto block = builder
				 .state ()
				 .account (key.pub)
				 .previous (0)
				 .representative (nano::dev::genesis_key.pub)
				 .balance (nano::Gxrb_ratio)
				 .link (123)
				 .sign (key.prv, key.pub)
				 .work (*node->work_generate_blocking (key.pub))
				 .build ();
	nano::block_hash hash (1);
	boost::property_tree::ptree request;
	request.put ("action", "work_generate");
	request.put ("hash", hash.to_string ());
	request.put ("json_block", "false");
	std::string json;
	block->serialize_json (json);
	request.put ("block", json);
	{
		auto response (wait_response (system, rpc_ctx, request));
		ASSERT_EQ (1, response.count ("error"));
		ASSERT_EQ (std::error_code (nano::error_rpc::block_root_mismatch).message (), response.get<std::string> ("error"));
	}
}

TEST (rpc, work_generate_block_ledger_epoch_2)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto wallet_id = node->wallets.first_wallet_id ();
	auto epoch1 = system.upgrade_genesis_epoch (*node, nano::epoch::epoch_1);
	ASSERT_NE (nullptr, epoch1);
	auto epoch2 = system.upgrade_genesis_epoch (*node, nano::epoch::epoch_2);
	ASSERT_NE (nullptr, epoch2);
	nano::keypair key;
	(void)node->wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);
	auto send_block (node->wallets.send_action (wallet_id, nano::dev::genesis_key.pub, key.pub, nano::Gxrb_ratio));
	ASSERT_NE (nullptr, send_block);
	nano::block_builder builder;
	auto block = builder
				 .state ()
				 .account (key.pub)
				 .previous (0)
				 .representative (nano::dev::genesis_key.pub)
				 .balance (nano::Gxrb_ratio)
				 .link (send_block->hash ())
				 .sign (key.prv, key.pub)
				 .work (0)
				 .build ();
	auto threshold (nano::dev::network_params.work.threshold (block->work_version (), nano::block_details (nano::epoch::epoch_2, false, true, false)));
	block->block_work_set (system.work_generate_limited (block->root ().as_block_hash (), 1, threshold - 1));
	nano::block_hash hash (block->root ().as_block_hash ());
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	request.put ("action", "work_generate");
	request.put ("hash", hash.to_string ());
	request.put ("json_block", "false");
	std::string json;
	block->serialize_json (json);
	request.put ("block", json);
	bool finished (false);
	auto iteration (0);
	while (!finished)
	{
		auto response (wait_response (system, rpc_ctx, request, 10s));
		auto work_text (response.get_optional<std::string> ("work"));
		ASSERT_TRUE (work_text.is_initialized ());
		uint64_t work;
		ASSERT_FALSE (nano::from_string_hex (*work_text, work));
		auto result_difficulty (nano::dev::network_params.work.difficulty (nano::work_version::work_1, hash, work));
		auto response_difficulty_text (response.get<std::string> ("difficulty"));
		uint64_t response_difficulty;
		ASSERT_FALSE (nano::from_string_hex (response_difficulty_text, response_difficulty));
		ASSERT_EQ (result_difficulty, response_difficulty);
		ASSERT_GE (result_difficulty, node->network_params.work.get_epoch_2_receive ());
		finished = result_difficulty < node->network_params.work.get_epoch_1 ();
		ASSERT_LT (++iteration, 200);
	}
}

TEST (rpc, work_cancel)
{
	nano::test::system system;
	auto node1 = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node1);
	nano::block_hash hash1 (1);
	boost::property_tree::ptree request1;
	request1.put ("action", "work_cancel");
	request1.put ("hash", hash1.to_string ());
	std::atomic<bool> done (false);
	system.deadline_set (10s);
	while (!done)
	{
		system.work.generate (nano::work_version::work_1, hash1, node1->network_params.work.get_base (), [&done] (boost::optional<uint64_t> work_a) {
			done = !work_a;
		});
		auto response1 (wait_response (system, rpc_ctx, request1));
		std::error_code ec;
		ASSERT_NO_ERROR (ec);
		std::string success (response1.get<std::string> ("success"));
		ASSERT_TRUE (success.empty ());
	}
}

TEST (rpc, work_peer_bad)
{
	nano::test::system system;
	auto node1 = add_ipc_enabled_node (system);
	auto & node2 = *system.add_node ();
	node2.config->work_peers.emplace_back (boost::asio::ip::address_v6::any ().to_string (), 0);
	auto const rpc_ctx = add_rpc (system, node1);
	nano::block_hash hash1 (1);
	std::atomic<uint64_t> work (0);
	node2.work_generate (nano::work_version::work_1, hash1, node2.network_params.work.get_base (), [&work] (std::optional<uint64_t> work_a) {
		ASSERT_TRUE (work_a.has_value ());
		work = work_a.value ();
	});
	ASSERT_TIMELY (5s, nano::dev::network_params.work.difficulty (nano::work_version::work_1, hash1, work) >= nano::dev::network_params.work.threshold_base (nano::work_version::work_1));
}

// Test disabled because it's failing intermittently.
// PR in which it got disabled: https://github.com/nanocurrency/nano-node/pull/3629
// Issue for investigating it: https://github.com/nanocurrency/nano-node/issues/3639
TEST (rpc, DISABLED_work_peer_one)
{
	nano::test::system system;
	auto node1 = add_ipc_enabled_node (system);
	auto & node2 = *system.add_node ();
	auto const rpc_ctx = add_rpc (system, node1);
	node2.config->work_peers.emplace_back (node1->network->endpoint ().address ().to_string (), rpc_ctx.rpc->listening_port ());
	nano::keypair key1;
	std::atomic<uint64_t> work (0);
	node2.work_generate (nano::work_version::work_1, key1.pub, node1->network_params.work.get_base (), [&work] (std::optional<uint64_t> work_a) {
		ASSERT_TRUE (work_a.has_value ());
		work = work_a.value ();
	});
	ASSERT_TIMELY (5s, nano::dev::network_params.work.difficulty (nano::work_version::work_1, key1.pub, work) >= nano::dev::network_params.work.threshold_base (nano::work_version::work_1));
}

// Test disabled because it's failing intermittently.
// PR in which it got disabled: https://github.com/nanocurrency/nano-node/pull/3629
// Issue for investigating it: https://github.com/nanocurrency/nano-node/issues/3636
TEST (rpc, DISABLED_work_peer_many)
{
	nano::test::system system1 (1);
	nano::test::system system2;
	nano::test::system system3 (1);
	nano::test::system system4 (1);
	auto & node1 (*system1.nodes[0]);
	auto node2 = add_ipc_enabled_node (system2);
	auto node3 = add_ipc_enabled_node (system3);
	auto node4 = add_ipc_enabled_node (system4);
	const auto rpc_ctx_2 = add_rpc (system2, node2);
	const auto rpc_ctx_3 = add_rpc (system3, node3);
	const auto rpc_ctx_4 = add_rpc (system4, node4);
	node1.config->work_peers.emplace_back (node2->network->endpoint ().address ().to_string (), rpc_ctx_2.rpc->listening_port ());
	node1.config->work_peers.emplace_back (node3->network->endpoint ().address ().to_string (), rpc_ctx_3.rpc->listening_port ());
	node1.config->work_peers.emplace_back (node4->network->endpoint ().address ().to_string (), rpc_ctx_4.rpc->listening_port ());

	std::array<std::atomic<uint64_t>, 10> works{};
	for (auto & work : works)
	{
		nano::keypair key1;
		node1.work_generate (nano::work_version::work_1, key1.pub, node1->network_params.work.get_base (), [&work] (std::optional<uint64_t> work_a) {
			work = work_a.value ();
		});
		while (nano::dev::network_params.work.difficulty (nano::work_version::work_1, key1.pub, work) < nano::dev::network_params.work.threshold_base (nano::work_version::work_1))
		{
			system1.poll ();
			system2.poll ();
			system3.poll ();
			system4.poll ();
		}
	}
	node1.stop ();
}

// Test disabled because it's failing intermittently.
// PR in which it got disabled: https://github.com/nanocurrency/nano-node/pull/3629
// Issue for investigating it: https://github.com/nanocurrency/nano-node/issues/3637
TEST (rpc, DISABLED_work_version_invalid)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node);
	nano::block_hash hash (1);
	boost::property_tree::ptree request;
	request.put ("action", "work_generate");
	request.put ("hash", hash.to_string ());
	request.put ("version", "work_invalid");
	{
		auto response (wait_response (system, rpc_ctx, request));
		ASSERT_EQ (1, response.count ("error"));
		ASSERT_EQ (std::error_code (nano::error_rpc::bad_work_version).message (), response.get<std::string> ("error"));
	}
	request.put ("action", "work_validate");
	{
		auto response (wait_response (system, rpc_ctx, request));
		ASSERT_EQ (1, response.count ("error"));
		ASSERT_EQ (std::error_code (nano::error_rpc::bad_work_version).message (), response.get<std::string> ("error"));
	}
}

TEST (rpc, block_count)
{
	{
		nano::test::system system;
		auto node1 = add_ipc_enabled_node (system);
		auto const rpc_ctx = add_rpc (system, node1);
		boost::property_tree::ptree request1;
		request1.put ("action", "block_count");
		{
			auto response1 (wait_response (system, rpc_ctx, request1));
			ASSERT_EQ ("1", response1.get<std::string> ("count"));
			ASSERT_EQ ("0", response1.get<std::string> ("unchecked"));
			ASSERT_EQ ("1", response1.get<std::string> ("cemented"));
		}
	}

	// Should be able to get all counts even when enable_control is false.
	{
		nano::test::system system;
		auto node1 = add_ipc_enabled_node (system);
		auto const rpc_ctx = add_rpc (system, node1);
		boost::property_tree::ptree request1;
		request1.put ("action", "block_count");
		{
			auto response1 (wait_response (system, rpc_ctx, request1));
			ASSERT_EQ ("1", response1.get<std::string> ("count"));
			ASSERT_EQ ("0", response1.get<std::string> ("unchecked"));
			ASSERT_EQ ("1", response1.get<std::string> ("cemented"));
		}
	}
}

TEST (rpc, block_count_pruning)
{
	nano::test::system system;
	auto & node0 = *system.add_node ();
	auto wallet_id = node0.wallets.first_wallet_id ();
	nano::node_config node_config = system.default_config ();
	node_config.enable_voting = false; // Remove after allowing pruned voting
	nano::node_flags node_flags;
	node_flags.set_enable_pruning (true);
	auto node1 = add_ipc_enabled_node (system, node_config, node_flags);
	auto latest (node1->latest (nano::dev::genesis_key.pub));
	nano::block_builder builder;
	auto send1 = builder
				 .send ()
				 .previous (latest)
				 .destination (nano::dev::genesis_key.pub)
				 .balance (nano::dev::constants.genesis_amount - nano::Gxrb_ratio)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (*node1->work_generate_blocking (latest))
				 .build ();
	node1->process_local (send1);
	auto receive1 = builder
					.receive ()
					.previous (send1->hash ())
					.source (send1->hash ())
					.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
					.work (*node1->work_generate_blocking (send1->hash ()))
					.build ();
	node1->process_local (receive1);
	(void)node0.wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);
	ASSERT_TIMELY (5s, node1->block_confirmed (receive1->hash ()));
	// Pruning action
	{
		auto transaction (node1->store.tx_begin_write ());
		ASSERT_EQ (1, node1->ledger.pruning_action (*transaction, send1->hash (), 1));
	}
	auto const rpc_ctx = add_rpc (system, node1);
	boost::property_tree::ptree request1;
	request1.put ("action", "block_count");
	{
		auto response1 (wait_response (system, rpc_ctx, request1));
		ASSERT_EQ ("3", response1.get<std::string> ("count"));
		ASSERT_EQ ("0", response1.get<std::string> ("unchecked"));
		ASSERT_EQ ("3", response1.get<std::string> ("cemented"));
		ASSERT_EQ ("2", response1.get<std::string> ("full"));
		ASSERT_EQ ("1", response1.get<std::string> ("pruned"));
	}
}

TEST (rpc, frontier_count)
{
	nano::test::system system;
	auto node1 = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node1);
	boost::property_tree::ptree request1;
	request1.put ("action", "frontier_count");
	auto response1 (wait_response (system, rpc_ctx, request1));
	ASSERT_EQ ("1", response1.get<std::string> ("count"));
}

TEST (rpc, account_count)
{
	nano::test::system system;
	auto node1 = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node1);
	boost::property_tree::ptree request1;
	request1.put ("action", "account_count");
	auto response1 (wait_response (system, rpc_ctx, request1));
	ASSERT_EQ ("1", response1.get<std::string> ("count"));
}

TEST (rpc, available_supply)
{
	nano::test::system system;
	auto node1 = add_ipc_enabled_node (system);
	auto wallet_id = node1->wallets.first_wallet_id ();
	auto const rpc_ctx = add_rpc (system, node1);
	boost::property_tree::ptree request1;
	request1.put ("action", "available_supply");
	auto response1 (wait_response (system, rpc_ctx, request1));
	ASSERT_EQ ("0", response1.get<std::string> ("available"));
	(void)node1->wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);
	nano::keypair key;
	auto block (node1->wallets.send_action (wallet_id, nano::dev::genesis_key.pub, key.pub, 1));
	auto response2 (wait_response (system, rpc_ctx, request1));
	ASSERT_EQ ("1", response2.get<std::string> ("available"));
	auto block2 (node1->wallets.send_action (wallet_id, nano::dev::genesis_key.pub, 0, 100)); // Sending to burning 0 account
	auto response3 (wait_response (system, rpc_ctx, request1, 10s));
	ASSERT_EQ ("1", response3.get<std::string> ("available"));
}

TEST (rpc, mrai_to_raw)
{
	nano::test::system system;
	auto node1 = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node1);
	boost::property_tree::ptree request1;
	request1.put ("action", "mrai_to_raw");
	request1.put ("amount", "1");
	auto response1 (wait_response (system, rpc_ctx, request1));
	ASSERT_EQ (nano::Mxrb_ratio.convert_to<std::string> (), response1.get<std::string> ("amount"));
}

TEST (rpc, mrai_from_raw)
{
	nano::test::system system;
	auto node1 = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node1);
	boost::property_tree::ptree request1;
	request1.put ("action", "mrai_from_raw");
	request1.put ("amount", nano::Mxrb_ratio.convert_to<std::string> ());
	auto response1 (wait_response (system, rpc_ctx, request1));
	ASSERT_EQ ("1", response1.get<std::string> ("amount"));
}

TEST (rpc, krai_to_raw)
{
	nano::test::system system;
	auto node1 = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node1);
	boost::property_tree::ptree request1;
	request1.put ("action", "krai_to_raw");
	request1.put ("amount", "1");
	auto response1 (wait_response (system, rpc_ctx, request1));
	ASSERT_EQ (nano::kxrb_ratio.convert_to<std::string> (), response1.get<std::string> ("amount"));
}

TEST (rpc, krai_from_raw)
{
	nano::test::system system;
	auto node1 = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node1);
	boost::property_tree::ptree request1;
	request1.put ("action", "krai_from_raw");
	request1.put ("amount", nano::kxrb_ratio.convert_to<std::string> ());
	auto response1 (wait_response (system, rpc_ctx, request1));
	ASSERT_EQ ("1", response1.get<std::string> ("amount"));
}

TEST (rpc, nano_to_raw)
{
	nano::test::system system;
	auto node1 = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node1);
	boost::property_tree::ptree request1;
	request1.put ("action", "nano_to_raw");
	request1.put ("amount", "1");
	auto response1 (wait_response (system, rpc_ctx, request1));
	ASSERT_EQ (nano::Mxrb_ratio.convert_to<std::string> (), response1.get<std::string> ("amount"));
}

TEST (rpc, raw_to_nano)
{
	nano::test::system system;
	auto node1 = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node1);
	boost::property_tree::ptree request1;
	request1.put ("action", "raw_to_nano");
	request1.put ("amount", nano::Mxrb_ratio.convert_to<std::string> ());
	auto response1 (wait_response (system, rpc_ctx, request1));
	ASSERT_EQ ("1", response1.get<std::string> ("amount"));
}

TEST (rpc, account_representative)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	request.put ("account", nano::dev::genesis_key.pub.to_account ());
	request.put ("action", "account_representative");
	auto response (wait_response (system, rpc_ctx, request));
	std::string account_text1 (response.get<std::string> ("representative"));
	ASSERT_EQ (account_text1, nano::dev::genesis_key.pub.to_account ());
}

TEST (rpc, account_representative_set)
{
	nano::test::system system;
	auto node = add_ipc_enabled_node (system);
	auto wallet_id = node->wallets.first_wallet_id ();
	(void)node->wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);

	// create a 2nd account and send it some nano
	nano::keypair key2;
	(void)node->wallets.insert_adhoc (wallet_id, key2.prv);
	auto key2_open_block_hash = node->wallets.send_sync (wallet_id, nano::dev::genesis_key.pub, key2.pub, node->config->receive_minimum.number ());
	ASSERT_TIMELY (5s, node->ledger.confirmed ().block_exists (*node->store.tx_begin_read (), key2_open_block_hash));
	auto key2_open_block = node->ledger.any ().block_get (*node->store.tx_begin_read (), key2_open_block_hash);
	ASSERT_EQ (nano::dev::genesis_key.pub, key2_open_block->representative_field ().value ());

	// now change the representative of key2 to be genesis
	auto const rpc_ctx = add_rpc (system, node);
	boost::property_tree::ptree request;
	request.put ("account", key2.pub.to_account ());
	request.put ("representative", key2.pub.to_account ());
	request.put ("wallet", node->wallets.first_wallet_id ().to_string ());
	request.put ("action", "account_representative_set");
	auto response (wait_response (system, rpc_ctx, request));
	std::string block_text1 (response.get<std::string> ("block"));

	// check that the rep change succeeded
	nano::block_hash hash;
	ASSERT_FALSE (hash.decode_hex (block_text1));
	ASSERT_FALSE (hash.is_zero ());
	auto block = node->ledger.any ().block_get (*node->store.tx_begin_read (), hash);
	ASSERT_NE (block, nullptr);
	ASSERT_TIMELY (5s, node->ledger.confirmed ().block_exists (*node->store.tx_begin_read (), hash));
	ASSERT_EQ (key2.pub, block->representative_field ().value ());
}
