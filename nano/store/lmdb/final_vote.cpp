#include <nano/store/lmdb/final_vote.hpp>
#include <nano/store/lmdb/lmdb.hpp>

namespace
{
nano::store::iterator<nano::qualified_root, nano::block_hash> to_iterator (rsnano::LmdbIteratorHandle * it_handle)
{
	if (it_handle == nullptr)
	{
		return nano::store::iterator<nano::qualified_root, nano::block_hash> (nullptr);
	}

	return nano::store::iterator<nano::qualified_root, nano::block_hash> (
	std::make_unique<nano::store::lmdb::iterator<nano::qualified_root, nano::block_hash>> (it_handle));
}
}

nano::store::lmdb::final_vote::final_vote (rsnano::LmdbFinalVoteStoreHandle * handle_a) :
	handle{ handle_a }
{
}

nano::store::lmdb::final_vote::~final_vote ()
{
	if (handle != nullptr)
		rsnano::rsn_lmdb_final_vote_store_destroy (handle);
}

bool nano::store::lmdb::final_vote::put (nano::store::write_transaction const & transaction, nano::qualified_root const & root, nano::block_hash const & hash)
{
	return rsnano::rsn_lmdb_final_vote_store_put (handle, transaction.get_rust_handle (), root.bytes.data (), hash.bytes.data ());
}

std::vector<nano::block_hash> nano::store::lmdb::final_vote::get (nano::store::transaction const & transaction, nano::root const & root_a)
{
	rsnano::BlockHashArrayDto dto;
	rsnano::rsn_lmdb_final_vote_store_get (handle, transaction.get_rust_handle (), root_a.bytes.data (), &dto);
	std::vector<nano::block_hash> result;
	for (auto i = 0; i < dto.count / 32; ++i)
	{
		nano::block_hash hash;
		std::copy (dto.data + (i * 32), dto.data + ((i + 1) * 32), std::begin (hash.bytes));
		result.push_back (hash);
	}
	rsnano::rsn_block_hash_array_destroy (&dto);
	return result;
}

void nano::store::lmdb::final_vote::del (nano::store::write_transaction const & transaction, nano::root const & root)
{
	rsnano::rsn_lmdb_final_vote_store_del (handle, transaction.get_rust_handle (), root.bytes.data ());
}

size_t nano::store::lmdb::final_vote::count (nano::store::transaction const & transaction_a) const
{
	return rsnano::rsn_lmdb_final_vote_store_count (handle, transaction_a.get_rust_handle ());
}

void nano::store::lmdb::final_vote::clear (nano::store::write_transaction const & transaction_a, nano::root const & root_a)
{
	del (transaction_a, root_a);
}

void nano::store::lmdb::final_vote::clear (nano::store::write_transaction const & transaction_a)
{
	rsnano::rsn_lmdb_final_vote_store_clear (handle, transaction_a.get_rust_handle ());
}

nano::store::iterator<nano::qualified_root, nano::block_hash> nano::store::lmdb::final_vote::begin (nano::store::transaction const & transaction, nano::qualified_root const & root) const
{
	auto it_handle{ rsnano::rsn_lmdb_final_vote_store_begin_at_root (handle, transaction.get_rust_handle (), root.bytes.data ()) };
	return to_iterator (it_handle);
}

nano::store::iterator<nano::qualified_root, nano::block_hash> nano::store::lmdb::final_vote::begin (nano::store::transaction const & transaction) const
{
	auto it_handle{ rsnano::rsn_lmdb_final_vote_store_begin (handle, transaction.get_rust_handle ()) };
	return to_iterator (it_handle);
}

nano::store::iterator<nano::qualified_root, nano::block_hash> nano::store::lmdb::final_vote::end () const
{
	return nano::store::iterator<nano::qualified_root, nano::block_hash> (nullptr);
}

