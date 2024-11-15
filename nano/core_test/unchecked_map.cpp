#include <nano/lib/blockbuilders.hpp>
#include <nano/lib/blocks.hpp>
#include <nano/lib/stats.hpp>
#include <nano/node/unchecked_map.hpp>
#include <nano/secure/common.hpp>
#include <nano/secure/utility.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

#include <memory>

using namespace std::chrono_literals;

namespace
{
unsigned max_unchecked_blocks = 65536;
}

// This test ensures the unchecked table is able to receive more than one block
TEST (unchecked, multiple)
{
	nano::test::system system{};
	nano::unchecked_map unchecked{ max_unchecked_blocks, system.stats, false };
	nano::block_builder builder;
	nano::keypair key;
	auto block = builder
				 .send ()
				 .previous (4)
				 .destination (1)
				 .balance (2)
				 .sign (key.prv, key.pub)
				 .work (5)
				 .build ();
	// Asserts the block wasn't added yet to the unchecked table
	auto block_listing1 = unchecked.get (block->previous ());
	ASSERT_TRUE (block_listing1.empty ());
	// Enqueues the first block
	unchecked.put (block->previous (), nano::unchecked_info (block));
	// Enqueues a second block
	unchecked.put (6, nano::unchecked_info (block));
	auto check_block_is_listed = [&] (nano::block_hash const & block_hash_a) {
		return unchecked.get (block_hash_a).size () > 0;
	};
	// Waits for and asserts the first block gets saved in the database
	ASSERT_TIMELY (5s, check_block_is_listed (block->previous ()));
	// Waits for and asserts the second block gets saved in the database
	ASSERT_TIMELY (5s, check_block_is_listed (6));
}

// This test ensures that a block can't occur twice in the unchecked table.
TEST (unchecked, double_put)
{
	nano::test::system system{};
	nano::unchecked_map unchecked{ max_unchecked_blocks, system.stats, false };
	nano::block_builder builder;
	nano::keypair key;
	auto block = builder
				 .send ()
				 .previous (4)
				 .destination (1)
				 .balance (2)
				 .sign (key.prv, key.pub)
				 .work (5)
				 .build ();
	// Asserts the block wasn't added yet to the unchecked table
	auto block_listing1 = unchecked.get (block->previous ());
	ASSERT_TRUE (block_listing1.empty ());
	// Enqueues the block to be saved in the unchecked table
	unchecked.put (block->previous (), nano::unchecked_info (block));
	// Enqueues the block again in an attempt to have it there twice
	unchecked.put (block->previous (), nano::unchecked_info (block));
	auto check_block_is_listed = [&] (nano::block_hash const & block_hash_a) {
		return unchecked.get (block_hash_a).size () > 0;
	};
	// Waits for and asserts the block was added at least once
	ASSERT_TIMELY (5s, check_block_is_listed (block->previous ()));
	// Asserts the block was added at most once -- this is objective of this test.
	auto block_listing2 = unchecked.get (block->previous ());
	ASSERT_EQ (block_listing2.size (), 1);
}

// Tests that recurrent get calls return the correct values
TEST (unchecked, multiple_get)
{
	nano::test::system system{};
	nano::unchecked_map unchecked{ max_unchecked_blocks, system.stats, false };
	// Instantiates three blocks
	nano::block_builder builder;
	nano::keypair key1;
	auto block1 = builder
				  .send ()
				  .previous (4)
				  .destination (1)
				  .balance (2)
				  .sign (key1.prv, key1.pub)
				  .work (5)
				  .build ();
	nano::keypair key2;
	auto block2 = builder
				  .send ()
				  .previous (3)
				  .destination (1)
				  .balance (2)
				  .sign (key2.prv, key2.pub)
				  .work (5)
				  .build ();
	nano::keypair key3;
	auto block3 = builder
				  .send ()
				  .previous (5)
				  .destination (1)
				  .balance (2)
				  .sign (key3.prv, key3.pub)
				  .work (5)
				  .build ();
	// Add the blocks' info to the unchecked table
	unchecked.put (block1->previous (), nano::unchecked_info (block1)); // unchecked1
	unchecked.put (block1->hash (), nano::unchecked_info (block1)); // unchecked2
	unchecked.put (block2->previous (), nano::unchecked_info (block2)); // unchecked3
	unchecked.put (block1->previous (), nano::unchecked_info (block2)); // unchecked1
	unchecked.put (block1->hash (), nano::unchecked_info (block2)); // unchecked2
	unchecked.put (block3->previous (), nano::unchecked_info (block3));
	unchecked.put (block3->hash (), nano::unchecked_info (block3)); // unchecked4
	unchecked.put (block1->previous (), nano::unchecked_info (block3)); // unchecked1

	// count the number of blocks in the unchecked table by counting them one by one
	// we cannot trust the count() method if the backend is rocksdb
	auto count_unchecked_blocks_one_by_one = [&unchecked] () {
		size_t count = 0;
		unchecked.for_each ([&count] (nano::unchecked_key const & key, nano::unchecked_info const & info) {
			++count;
		});
		return count;
	};

	// Waits for the blocks to get saved in the database
	ASSERT_TIMELY_EQ (5s, 8, count_unchecked_blocks_one_by_one ());

	std::vector<nano::block_hash> unchecked1;
	// Asserts the entries will be found for the provided key
	auto unchecked1_blocks = unchecked.get (block1->previous ());
	ASSERT_EQ (unchecked1_blocks.size (), 3);
	for (auto & i : unchecked1_blocks)
	{
		unchecked1.push_back (i.get_block ()->hash ());
	}
	// Asserts the payloads where correclty saved
	ASSERT_TRUE (std::find (unchecked1.begin (), unchecked1.end (), block1->hash ()) != unchecked1.end ());
	ASSERT_TRUE (std::find (unchecked1.begin (), unchecked1.end (), block2->hash ()) != unchecked1.end ());
	ASSERT_TRUE (std::find (unchecked1.begin (), unchecked1.end (), block3->hash ()) != unchecked1.end ());
	std::vector<nano::block_hash> unchecked2;
	// Asserts the entries will be found for the provided key
	auto unchecked2_blocks = unchecked.get (block1->hash ());
	ASSERT_EQ (unchecked2_blocks.size (), 2);
	for (auto & i : unchecked2_blocks)
	{
		unchecked2.push_back (i.get_block ()->hash ());
	}
	// Asserts the payloads where correctly saved
	ASSERT_TRUE (std::find (unchecked2.begin (), unchecked2.end (), block1->hash ()) != unchecked2.end ());
	ASSERT_TRUE (std::find (unchecked2.begin (), unchecked2.end (), block2->hash ()) != unchecked2.end ());
	// Asserts the entry is found by the key and the payload is saved
	auto unchecked3 = unchecked.get (block2->previous ());
	ASSERT_EQ (unchecked3.size (), 1);
	ASSERT_EQ (unchecked3[0].get_block ()->hash (), block2->hash ());
	// Asserts the entry is found by the key and the payload is saved
	auto unchecked4 = unchecked.get (block3->hash ());
	ASSERT_EQ (unchecked4.size (), 1);
	ASSERT_EQ (unchecked4[0].get_block ()->hash (), block3->hash ());
	// Asserts no entry is found for a block that wasn't added
	auto unchecked5 = unchecked.get (block2->hash ());
	ASSERT_EQ (unchecked5.size (), 0);
}
