#include <nano/lib/blocks.hpp>
#include <nano/lib/logging.hpp>
#include <nano/lib/stats.hpp>
#include <nano/lib/threading.hpp>
#include <nano/node/active_elections.hpp>
#include <nano/node/election.hpp>
#include <nano/node/make_store.hpp>
#include <nano/node/scheduler/component.hpp>
#include <nano/node/scheduler/priority.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

using namespace std::chrono_literals;

TEST (ledger, unchecked_open)
{
	nano::test::system system (1);
	auto & node1 (*system.nodes[0]);
	nano::keypair destination;
	nano::block_builder builder;
	auto send1 = builder
				 .state ()
				 .account (nano::dev::genesis_key.pub)
				 .previous (nano::dev::genesis->hash ())
				 .representative (nano::dev::genesis_key.pub)
				 .balance (nano::dev::constants.genesis_amount - nano::Gxrb_ratio)
				 .link (destination.pub)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (0)
				 .build ();
	node1.work_generate_blocking (*send1);
	auto open1 = builder
				 .open ()
				 .source (send1->hash ())
				 .representative (destination.pub)
				 .account (destination.pub)
				 .sign (destination.prv, destination.pub)
				 .work (0)
				 .build ();
	node1.work_generate_blocking (*open1);
	// Invalid signature for open block
	auto open2 = builder
				 .open ()
				 .source (send1->hash ())
				 .representative (nano::dev::genesis_key.pub)
				 .account (destination.pub)
				 .sign (destination.prv, destination.pub)
				 .work (0)
				 .build ();
	node1.work_generate_blocking (*open2);
	auto sig{ open2->block_signature () };
	sig.bytes[0] ^= 1;
	open2->signature_set (sig);
	node1.block_processor.add (open2); // Insert open2 in to the queue before open1
	node1.block_processor.add (open1);
	{
		// Waits for the last blocks to pass through block_processor and unchecked.put queues
		ASSERT_TIMELY_EQ (5s, 1, node1.unchecked.count ());
		// When open1 existists in unchecked, we know open2 has been processed.
		auto blocks = node1.unchecked.get (open1->source_field ().value ());
		ASSERT_EQ (blocks.size (), 1);
	}
	node1.block_processor.add (send1);
	// Waits for the send1 block to pass through block_processor and unchecked.put queues
	ASSERT_TIMELY (5s, node1.ledger.any ().block_exists (*node1.store.tx_begin_read (), open1->hash ()));
	ASSERT_EQ (0, node1.unchecked.count ());
}

TEST (ledger, unchecked_receive)
{
	nano::test::system system{ 1 };
	auto & node1 = *system.nodes[0];
	nano::keypair destination{};
	nano::block_builder builder;
	auto send1 = builder
				 .state ()
				 .account (nano::dev::genesis_key.pub)
				 .previous (nano::dev::genesis->hash ())
				 .representative (nano::dev::genesis_key.pub)
				 .balance (nano::dev::constants.genesis_amount - nano::Gxrb_ratio)
				 .link (destination.pub)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (0)
				 .build ();
	node1.work_generate_blocking (*send1);
	auto send2 = builder
				 .state ()
				 .account (nano::dev::genesis_key.pub)
				 .previous (send1->hash ())
				 .representative (nano::dev::genesis_key.pub)
				 .balance (nano::dev::constants.genesis_amount - 2 * nano::Gxrb_ratio)
				 .link (destination.pub)
				 .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				 .work (0)
				 .build ();
	node1.work_generate_blocking (*send2);
	auto open1 = builder
				 .open ()
				 .source (send1->hash ())
				 .representative (destination.pub)
				 .account (destination.pub)
				 .sign (destination.prv, destination.pub)
				 .work (0)
				 .build ();
	node1.work_generate_blocking (*open1);
	auto receive1 = builder
					.receive ()
					.previous (open1->hash ())
					.source (send2->hash ())
					.sign (destination.prv, destination.pub)
					.work (0)
					.build ();
	node1.work_generate_blocking (*receive1);
	node1.block_processor.add (send1);
	node1.block_processor.add (receive1);
	auto check_block_is_listed = [&] (nano::store::transaction const & transaction_a, nano::block_hash const & block_hash_a) {
		return !node1.unchecked.get (block_hash_a).empty ();
	};
	// Previous block for receive1 is unknown, signature cannot be validated
	{
		// Waits for the last blocks to pass through block_processor and unchecked.put queues
		ASSERT_TIMELY (15s, check_block_is_listed (*node1.store.tx_begin_read (), receive1->previous ()));
		auto blocks = node1.unchecked.get (receive1->previous ());
		ASSERT_EQ (blocks.size (), 1);
	}
	// Waits for the open1 block to pass through block_processor and unchecked.put queues
	node1.block_processor.add (open1);
	ASSERT_TIMELY (15s, check_block_is_listed (*node1.store.tx_begin_read (), receive1->source_field ().value ()));
	// Previous block for receive1 is known, signature was validated
	{
		auto transaction = node1.store.tx_begin_read ();
		auto blocks (node1.unchecked.get (receive1->source_field ().value ()));
		ASSERT_EQ (blocks.size (), 1);
	}
	node1.block_processor.add (send2);
	ASSERT_TIMELY (10s, node1.ledger.any ().block_exists (*node1.store.tx_begin_read (), receive1->hash ()));
	ASSERT_EQ (0, node1.unchecked.count ());
}
