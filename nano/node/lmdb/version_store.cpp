#include <nano/node/lmdb/lmdb.hpp>
#include <nano/node/lmdb/version_store.hpp>

nano::lmdb::version_store::version_store (nano::lmdb::store & store_a) :
	store{ store_a },
	handle{ rsnano::rsn_lmdb_version_store_create (store_a.env ().handle) } {};

nano::lmdb::version_store::~version_store ()
{
	rsnano::rsn_lmdb_version_store_destroy (handle);
}

void nano::lmdb::version_store::put (nano::write_transaction const & transaction_a, int version)
{
	rsnano::rsn_lmdb_version_store_put (handle, transaction_a.get_rust_handle (), version);
}

int nano::lmdb::version_store::get (nano::transaction const & transaction_a) const
{
	return rsnano::rsn_lmdb_version_store_get (handle, transaction_a.get_rust_handle ());
}

MDB_dbi nano::lmdb::version_store::table_handle () const
{
	return rsnano::rsn_lmdb_version_store_table_handle (handle);
}

void nano::lmdb::version_store::set_table_handle (MDB_dbi dbi)
{
	rsnano::rsn_lmdb_version_store_set_table_handle (handle, dbi);
}
