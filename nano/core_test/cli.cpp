#include <nano/lib/cli.hpp>
#include <nano/node/cli.hpp>
#include <nano/secure/utility.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

#include <boost/program_options.hpp>

using namespace std::chrono_literals;

namespace
{
std::string call_cli_command (boost::program_options::variables_map const & vm);
}

TEST (cli, config_override_parsing)
{
	std::vector<nano::config_key_value_pair> key_value_pairs;
	auto config_overrides = nano::config_overrides (key_value_pairs);
	ASSERT_TRUE (config_overrides.empty ());
	key_value_pairs.push_back ({ "key", "value" });
	config_overrides = nano::config_overrides (key_value_pairs);
	ASSERT_EQ (config_overrides[0], "key=\"value\"");
	key_value_pairs.push_back ({ "node.online_weight_minimum", "40000000000000000000000000000000000000" });
	config_overrides = nano::config_overrides (key_value_pairs);
	ASSERT_EQ (config_overrides[1], "node.online_weight_minimum=\"40000000000000000000000000000000000000\"");

	// Should add this as it contains escaped quotes, and make sure these are not escaped again
	key_value_pairs.push_back ({ "key", "\"value\"" });
	config_overrides = nano::config_overrides (key_value_pairs);
	ASSERT_EQ (config_overrides[2], "key=\"value\"");
	ASSERT_EQ (config_overrides.size (), 3);

	// Try it with arrays, with and without escaped quotes
	key_value_pairs.push_back ({ "node.work_peers", "[127.0.0.1:7000,\"128.0.0.1:50000\"]" });
	config_overrides = nano::config_overrides (key_value_pairs);
	ASSERT_EQ (config_overrides[3], "node.work_peers=[\"127.0.0.1:7000\",\"128.0.0.1:50000\"]");
	ASSERT_EQ (config_overrides.size (), 4);
}

namespace
{
std::string call_cli_command (boost::program_options::variables_map const & vm)
{
	std::stringstream ss;
	nano::test::cout_redirect redirect (ss.rdbuf ());

	// Execute CLI command. This populates the stringstream with a string like: "Private: 123\n Public: 456\n Account: nano_123"
	auto ec = nano::handle_node_options (vm);
	release_assert (!static_cast<bool> (ec));
	return ss.str ();
}
}
