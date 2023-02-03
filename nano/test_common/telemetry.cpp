#include <nano/node/common.hpp>
#include <nano/node/messages.hpp>
#include <nano/node/node.hpp>
#include <nano/test_common/telemetry.hpp>

#include <gtest/gtest.h>

namespace
{
void compare_telemetry_data_impl (const nano::telemetry_data & data_a, const nano::telemetry_data & data_b, bool & result)
{
	ASSERT_EQ (data_a.get_block_count (), data_b.get_block_count ());
	ASSERT_EQ (data_a.get_cemented_count (), data_b.get_cemented_count ());
	ASSERT_EQ (data_a.get_bandwidth_cap (), data_b.get_bandwidth_cap ());
	ASSERT_EQ (data_a.get_peer_count (), data_b.get_peer_count ());
	ASSERT_EQ (data_a.get_protocol_version (), data_b.get_protocol_version ());
	ASSERT_EQ (data_a.get_unchecked_count (), data_b.get_unchecked_count ());
	ASSERT_EQ (data_a.get_account_count (), data_b.get_account_count ());
	ASSERT_LE (data_a.get_uptime (), data_b.get_uptime ());
	ASSERT_EQ (data_a.get_genesis_block (), data_b.get_genesis_block ());
	ASSERT_EQ (data_a.get_major_version (), nano::get_major_node_version ());
	ASSERT_EQ (data_a.get_minor_version (), nano::get_minor_node_version ());
	ASSERT_EQ (data_a.get_patch_version (), nano::get_patch_node_version ());
	ASSERT_EQ (data_a.get_pre_release_version (), nano::get_pre_release_node_version ());
	ASSERT_EQ (data_a.get_maker (), static_cast<std::underlying_type_t<nano::telemetry_maker>> (nano::telemetry_maker::nf_node));
	ASSERT_GT (data_a.get_timestamp (), std::chrono::system_clock::now () - std::chrono::seconds (100));
	ASSERT_EQ (data_a.get_active_difficulty (), data_b.get_active_difficulty ());
	ASSERT_EQ (data_a.get_unknown_data (), std::vector<uint8_t>{});
	result = true;
}
}

bool nano::test::compare_telemetry_data (const nano::telemetry_data & data_a, const nano::telemetry_data & data_b)
{
	bool result = false;
	compare_telemetry_data_impl (data_a, data_b, result);
	return result;
}

namespace
{
void compare_telemetry_impl (const nano::telemetry_data & data, nano::node const & node, bool & result)
{
	ASSERT_FALSE (data.validate_signature ());
	ASSERT_EQ (data.get_node_id (), node.node_id.pub);
	ASSERT_TRUE (nano::test::compare_telemetry_data (data, node.local_telemetry ()));

	result = true;
}
}

bool nano::test::compare_telemetry (const nano::telemetry_data & data, const nano::node & node)
{
	bool result = false;
	compare_telemetry_impl (data, node, result);
	return result;
}