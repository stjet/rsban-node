#include "nano/lib/rsnano.hpp"

#include <nano/lib/blocks.hpp>
#include <nano/lib/utility.hpp>
#include <nano/node/scheduler/buckets.hpp>

#include <string>

nano::scheduler::buckets::value_type::value_type (uint64_t time, std::shared_ptr<nano::block> block) :
	handle (rsnano::rsn_prioritization_create_value_type (time, block->get_handle ()))
{
}

nano::scheduler::buckets::value_type::~value_type ()
{
	rsnano::rsn_prioritization_drop_value_type (handle);
}

uint64_t nano::scheduler::buckets::value_type::get_time () const
{
	return rsnano::rsn_prioritization_get_value_type_time (handle);
}

std::shared_ptr<nano::block> nano::scheduler::buckets::value_type::get_block () const
{
	auto block_handle = rsnano::rsn_prioritization_get_value_type_block (handle);
	return block_handle_to_block (block_handle);
}

bool nano::scheduler::buckets::value_type::operator< (value_type const & other_a) const
{
	return rsnano::rsn_prioritization_value_type_cmp (handle, other_a.handle) < 0;
}

bool nano::scheduler::buckets::value_type::operator== (value_type const & other_a) const
{
	return rsnano::rsn_prioritization_value_type_cmp (handle, other_a.handle) == 0;
}

/**
 * Prioritization constructor, construct a container containing approximately 'maximum' number of blocks.
 * @param maximum number of blocks that this container can hold, this is a soft and approximate limit.
 */
nano::scheduler::buckets::buckets (uint64_t maximum) :
	handle (rsnano::rsn_prioritization_create (maximum))
{
}

nano::scheduler::buckets::~buckets ()
{
	rsnano::rsn_prioritization_destroy (handle);
}

std::size_t nano::scheduler::buckets::index (nano::uint128_t const & balance) const
{
	nano::amount balance_amount{ balance };
	return rsnano::rsn_prioritization_index (handle, balance_amount.bytes.data ());
}

/**
 * Push a block and its associated time into the prioritization container.
 * The time is given here because sideband might not exist in the case of state blocks.
 */
void nano::scheduler::buckets::push (uint64_t time, std::shared_ptr<nano::block> block, nano::amount const & priority)
{
	rsnano::rsn_prioritization_push (handle, time, block->get_handle (), priority.bytes.data ());
}

/** Return the highest priority block of the current bucket */
std::shared_ptr<nano::block> nano::scheduler::buckets::top () const
{
	return nano::block_handle_to_block (rsn_prioritization_top (handle));
}

/** Pop the current block from the container and seek to the next block, if it exists */
void nano::scheduler::buckets::pop ()
{
	rsnano::rsn_prioritization_pop (handle);
}

/** Returns the total number of blocks in buckets */
std::size_t nano::scheduler::buckets::size () const
{
	return rsnano::rsn_prioritization_size (handle);
}

/** Returns number of buckets, 62 by default */
std::size_t nano::scheduler::buckets::bucket_count () const
{
	return rsnano::rsn_prioritization_bucket_count (handle);
}

/** Returns number of items in bucket with index 'index' */
std::size_t nano::scheduler::buckets::bucket_size (std::size_t index) const
{
	return rsnano::rsn_prioritization_bucket_size (handle, index);
}

/** Returns true if all buckets are empty */
bool nano::scheduler::buckets::empty () const
{
	return rsnano::rsn_prioritization_empty (handle);
}

/** Print the state of the class in stderr */
void nano::scheduler::buckets::dump () const
{
	rsnano::rsn_prioritization_dump (handle);
}

std::unique_ptr<nano::container_info_component> nano::scheduler::buckets::collect_container_info (std::string const & name)
{
	auto composite = std::make_unique<container_info_composite> (name);
	auto size = rsnano::rsn_prioritization_bucket_count (handle);
	for (auto i = 0; i < size; ++i)
	{
		auto const & bucket_size = rsnano::rsn_prioritization_bucket_size (handle, i);
		composite->add_component (std::make_unique<container_info_leaf> (container_info{ std::to_string (i), bucket_size, 0 }));
	}
	return composite;
}
