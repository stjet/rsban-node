#include <nano/store/lmdb/block.hpp>
#include <nano/store/lmdb/lmdb.hpp>

nano::store::lmdb::block::block (rsnano::LmdbBlockStoreHandle * handle_a) :
	handle{ handle_a }
{
}

nano::store::lmdb::block::~block ()
{
	if (handle != nullptr)
		rsnano::rsn_lmdb_block_store_destroy (handle);
}

void nano::store::lmdb::block::put (nano::store::write_transaction const & transaction, nano::block_hash const & hash, nano::block const & block)
{
	rsnano::rsn_lmdb_block_store_put (handle, transaction.get_rust_handle (), hash.bytes.data (), block.get_handle ());
}

void nano::store::lmdb::block::raw_put (nano::store::write_transaction const & transaction_a, std::vector<uint8_t> const & data, nano::block_hash const & hash_a)
{
	rsnano::rsn_lmdb_block_store_raw_put (handle, transaction_a.get_rust_handle (), data.data (), data.size (), hash_a.bytes.data ());
}

nano::block_hash nano::store::lmdb::block::successor (nano::store::transaction const & transaction_a, nano::block_hash const & hash_a) const
{
	nano::block_hash result;
	rsnano::rsn_lmdb_block_store_successor (handle, transaction_a.get_rust_handle (), hash_a.bytes.data (), result.bytes.data ());
	return result;
}

void nano::store::lmdb::block::successor_clear (nano::store::write_transaction const & transaction, nano::block_hash const & hash)
{
	rsnano::rsn_lmdb_block_store_successor_clear (handle, transaction.get_rust_handle (), hash.bytes.data ());
}

std::shared_ptr<nano::block> nano::store::lmdb::block::get (nano::store::transaction const & transaction, nano::block_hash const & hash) const
{
	auto block_handle = rsnano::rsn_lmdb_block_store_get (handle, transaction.get_rust_handle (), hash.bytes.data ());
	return nano::block_handle_to_block (block_handle);
}

std::shared_ptr<nano::block> nano::store::lmdb::block::random (nano::store::transaction const & transaction)
{
	std::shared_ptr<nano::block> result;
	auto block_handle = rsnano::rsn_lmdb_block_store_random (handle, transaction.get_rust_handle ());
	if (block_handle != nullptr)
	{
		result = std::move (nano::block_handle_to_block (block_handle));
	}
	return result;
}

void nano::store::lmdb::block::del (nano::store::write_transaction const & transaction_a, nano::block_hash const & hash_a)
{
	rsnano::rsn_lmdb_block_store_del (handle, transaction_a.get_rust_handle (), hash_a.bytes.data ());
}

bool nano::store::lmdb::block::exists (nano::store::transaction const & transaction, nano::block_hash const & hash)
{
	return rsnano::rsn_lmdb_block_store_exists (handle, transaction.get_rust_handle (), hash.bytes.data ());
}

uint64_t nano::store::lmdb::block::count (nano::store::transaction const & transaction_a)
{
	return rsnano::rsn_lmdb_block_store_count (handle, transaction_a.get_rust_handle ());
}
