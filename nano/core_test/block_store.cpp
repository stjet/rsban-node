#include <nano/crypto_lib/random_pool.hpp>
#include <nano/lib/lmdbconfig.hpp>
#include <nano/lib/logger_mt.hpp>
#include <nano/lib/stats.hpp>
#include <nano/lib/utility.hpp>
#include <nano/lib/work.hpp>
#include <nano/node/common.hpp>
#include <nano/node/make_store.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/secure/utility.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

#include <cstdlib>
#include <fstream>
#include <unordered_set>
#include <vector>

using namespace std::chrono_literals;

TEST (block_store, empty_bootstrap)
{
	nano::test::system system{};
	nano::unchecked_map unchecked{ system.stats, false };
	size_t count = 0;
	unchecked.for_each ([&count] (nano::unchecked_key const & key, nano::unchecked_info const & info) {
		++count;
	});
	ASSERT_EQ (count, 0);
}
