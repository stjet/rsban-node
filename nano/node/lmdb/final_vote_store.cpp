#include <nano/node/lmdb/final_vote_store.hpp>
#include <nano/node/lmdb/lmdb.hpp>
#include <nano/secure/parallel_traversal.hpp>

namespace
{
nano::store_iterator<nano::qualified_root, nano::block_hash> to_iterator (rsnano::LmdbIteratorHandle * it_handle)
{
	if (it_handle == nullptr)
	{
		return nano::store_iterator<nano::qualified_root, nano::block_hash> (nullptr);
	}

	return nano::store_iterator<nano::qualified_root, nano::block_hash> (
	std::make_unique<nano::mdb_iterator<nano::qualified_root, nano::block_hash>> (it_handle));
}
}

nano::lmdb::final_vote_store::final_vote_store (nano::lmdb::store & store) :
	store{ store }
{
	handle = rsnano::rsn_lmdb_final_vote_store_create (store.env ().handle);
};

nano::lmdb::final_vote_store::~final_vote_store ()
{
	rsnano::rsn_lmdb_final_vote_store_destroy (handle);
}

bool nano::lmdb::final_vote_store::put (nano::write_transaction const & transaction, nano::qualified_root const & root, nano::block_hash const & hash)
{
	return rsnano::rsn_lmdb_final_vote_store_put (handle, transaction.get_rust_handle (), root.bytes.data (), hash.bytes.data ());
}

std::vector<nano::block_hash> nano::lmdb::final_vote_store::get (nano::transaction const & transaction, nano::root const & root_a)
{
	//	std::vector<nano::block_hash> result;
	//	nano::qualified_root key_start{ root_a.raw, 0 };
	//	for (auto i = begin (transaction, key_start), n = end (); i != n && nano::qualified_root{ i->first }.root () == root_a; ++i)
	//	{
	//		result.push_back (i->second);
	//
	//
	//	}
	//
	//	return result;

	rsnano::BlockHashArrayDto dto;
	rsnano::rsn_lmdb_final_vote_store_begin_get (handle, transaction.get_rust_handle (), root_a.bytes.data (), &dto);
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

void nano::lmdb::final_vote_store::del (nano::write_transaction const & transaction, nano::root const & root)
{
	std::vector<nano::qualified_root> final_vote_qualified_roots;
	for (auto i = begin (transaction, nano::qualified_root{ root.raw, 0 }), n = end (); i != n && nano::qualified_root{ i->first }.root () == root; ++i)
	{
		final_vote_qualified_roots.push_back (i->first);
	}

	for (auto & final_vote_qualified_root : final_vote_qualified_roots)
	{
		auto status = store.del (transaction, tables::final_votes, final_vote_qualified_root);
		store.release_assert_success (status);
	}
}

size_t nano::lmdb::final_vote_store::count (nano::transaction const & transaction_a) const
{
	return store.count (transaction_a, tables::final_votes);
}

void nano::lmdb::final_vote_store::clear (nano::write_transaction const & transaction_a, nano::root const & root_a)
{
	del (transaction_a, root_a);
}

void nano::lmdb::final_vote_store::clear (nano::write_transaction const & transaction_a)
{
	store.drop (transaction_a, nano::tables::final_votes);
}

nano::store_iterator<nano::qualified_root, nano::block_hash> nano::lmdb::final_vote_store::begin (nano::transaction const & transaction, nano::qualified_root const & root) const
{
	auto it_handle{ rsnano::rsn_lmdb_final_vote_store_begin_at_root (handle, transaction.get_rust_handle (), root.bytes.data ()) };
	return to_iterator (it_handle);
}

nano::store_iterator<nano::qualified_root, nano::block_hash> nano::lmdb::final_vote_store::begin (nano::transaction const & transaction) const
{
	auto it_handle{ rsnano::rsn_lmdb_final_vote_store_begin (handle, transaction.get_rust_handle ()) };
	return to_iterator (it_handle);
}

nano::store_iterator<nano::qualified_root, nano::block_hash> nano::lmdb::final_vote_store::end () const
{
	return nano::store_iterator<nano::qualified_root, nano::block_hash> (nullptr);
}

void nano::lmdb::final_vote_store::for_each_par (std::function<void (nano::read_transaction const &, nano::store_iterator<nano::qualified_root, nano::block_hash>, nano::store_iterator<nano::qualified_root, nano::block_hash>)> const & action_a) const
{
	parallel_traversal<nano::uint512_t> (
	[&action_a, this] (nano::uint512_t const & start, nano::uint512_t const & end, bool const is_last) {
		auto transaction (this->store.tx_begin_read ());
		action_a (*transaction, this->begin (*transaction, start), !is_last ? this->begin (*transaction, end) : this->end ());
	});
}

MDB_dbi nano::lmdb::final_vote_store::table_handle () const
{
	return rsnano::rsn_lmdb_final_vote_store_table_handle (handle);
}

void nano::lmdb::final_vote_store::set_table_handle (MDB_dbi dbi)
{
	rsnano::rsn_lmdb_final_vote_store_set_table_handle (handle, dbi);
}
