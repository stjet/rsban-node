#include <nano/store/lmdb/lmdb.hpp>
#include <nano/store/lmdb/pruned.hpp>

namespace
{
nano::store::iterator<nano::block_hash, std::nullptr_t> to_iterator (rsnano::LmdbIteratorHandle * it_handle)
{
	if (it_handle == nullptr)
	{
		return { nullptr };
	}

	return { std::make_unique<nano::store::lmdb::iterator<nano::block_hash, std::nullptr_t>> (it_handle) };
}
}

nano::store::lmdb::pruned::pruned (rsnano::LmdbPrunedStoreHandle * handle_a) :
	handle{ handle_a }
{
}

nano::store::lmdb::pruned::~pruned ()
{
	if (handle != nullptr)
		rsnano::rsn_lmdb_pruned_store_destroy (handle);
}

void nano::store::lmdb::pruned::put (nano::store::write_transaction const & transaction_a, nano::block_hash const & hash_a)
{
	rsnano::rsn_lmdb_pruned_store_put (handle, transaction_a.get_rust_handle (), hash_a.bytes.data ());
}

void nano::store::lmdb::pruned::del (nano::store::write_transaction const & transaction_a, nano::block_hash const & hash_a)
{
	rsnano::rsn_lmdb_pruned_store_del (handle, transaction_a.get_rust_handle (), hash_a.bytes.data ());
}

bool nano::store::lmdb::pruned::exists (nano::store::transaction const & transaction_a, nano::block_hash const & hash_a) const
{
	return rsnano::rsn_lmdb_pruned_store_exists (handle, transaction_a.get_rust_handle (), hash_a.bytes.data ());
}

nano::block_hash nano::store::lmdb::pruned::random (nano::store::transaction const & transaction)
{
	nano::block_hash random_hash;
	rsnano::rsn_lmdb_pruned_store_random (handle, transaction.get_rust_handle (), random_hash.bytes.data ());
	return random_hash;
}

size_t nano::store::lmdb::pruned::count (nano::store::transaction const & transaction_a) const
{
	return rsnano::rsn_lmdb_pruned_store_count (handle, transaction_a.get_rust_handle ());
}

void nano::store::lmdb::pruned::clear (nano::store::write_transaction const & transaction_a)
{
	return rsnano::rsn_lmdb_pruned_store_clear (handle, transaction_a.get_rust_handle ());
}

nano::store::iterator<nano::block_hash, std::nullptr_t> nano::store::lmdb::pruned::begin (nano::store::transaction const & transaction, nano::block_hash const & hash) const
{
	auto it_handle{ rsnano::rsn_lmdb_pruned_store_begin_at_hash (handle, transaction.get_rust_handle (), hash.bytes.data ()) };
	return to_iterator (it_handle);
}

nano::store::iterator<nano::block_hash, std::nullptr_t> nano::store::lmdb::pruned::begin (nano::store::transaction const & transaction) const
{
	auto it_handle{ rsnano::rsn_lmdb_pruned_store_begin (handle, transaction.get_rust_handle ()) };
	return to_iterator (it_handle);
}

nano::store::iterator<nano::block_hash, std::nullptr_t> nano::store::lmdb::pruned::end () const
{
	return nano::store::iterator<nano::block_hash, std::nullptr_t> (nullptr);
}

namespace
{
void for_each_par_wrapper (void * context, rsnano::TransactionHandle * txn_handle, rsnano::LmdbIteratorHandle * begin_handle, rsnano::LmdbIteratorHandle * end_handle)
{
	auto action = static_cast<std::function<void (nano::store::read_transaction const &, nano::store::iterator<nano::block_hash, std::nullptr_t>, nano::store::iterator<nano::block_hash, std::nullptr_t>)> const *> (context);
	nano::store::lmdb::read_transaction_impl txn{ txn_handle };
	auto begin{ to_iterator (begin_handle) };
	auto end{ to_iterator (end_handle) };
	(*action) (txn, std::move (begin), std::move (end));
}
void for_each_par_delete_context (void * context)
{
}
}

void nano::store::lmdb::pruned::for_each_par (std::function<void (nano::store::read_transaction const &, nano::store::iterator<nano::block_hash, std::nullptr_t>, nano::store::iterator<nano::block_hash, std::nullptr_t>)> const & action_a) const
{
	auto context = (void *)&action_a;
	rsnano::rsn_lmdb_pruned_store_for_each_par (handle, for_each_par_wrapper, context, for_each_par_delete_context);
}
