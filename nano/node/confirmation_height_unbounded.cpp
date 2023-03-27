#include "nano/lib/rsnanoutils.hpp"

#include <nano/lib/stats.hpp>
#include <nano/node/confirmation_height_unbounded.hpp>
#include <nano/node/logging.hpp>
#include <nano/node/write_database_queue.hpp>
#include <nano/secure/ledger.hpp>

nano::block_hash_vec::block_hash_vec () :
	handle{ rsnano::rsn_block_hash_vec_create () }
{
}

nano::block_hash_vec::block_hash_vec (rsnano::BlockHashVecHandle * handle_a) :
	handle{ handle_a }
{
}

nano::block_hash_vec::block_hash_vec (nano::block_hash_vec const & other_a) :
	handle{ rsnano::rsn_block_hash_vec_clone (other_a.handle) }
{
}

nano::block_hash_vec::~block_hash_vec ()
{
	rsnano::rsn_block_hash_vec_destroy (handle);
}
nano::block_hash_vec & nano::block_hash_vec::operator= (block_hash_vec const & other_a)
{
	rsnano::rsn_block_hash_vec_destroy (handle);
	handle = rsnano::rsn_block_hash_vec_clone (other_a.handle);
	return *this;
}
bool nano::block_hash_vec::empty () const
{
	return size () == 0;
}
size_t nano::block_hash_vec::size () const
{
	return rsnano::rsn_block_hash_vec_size (handle);
}
void nano::block_hash_vec::push_back (const nano::block_hash & hash)
{
	rsnano::rsn_block_hash_vec_push (handle, hash.bytes.data ());
}
void nano::block_hash_vec::clear ()
{
	rsnano::rsn_block_hash_vec_clear (handle);
}
void nano::block_hash_vec::assign (block_hash_vec const & source_a, size_t start, size_t end)
{
	rsnano::rsn_block_hash_vec_assign_range (handle, source_a.handle, start, end);
}
void nano::block_hash_vec::truncate (size_t new_size_a)
{
	rsnano::rsn_block_hash_vec_truncate (handle, new_size_a);
}
