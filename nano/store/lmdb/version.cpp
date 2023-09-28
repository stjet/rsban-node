#include <nano/store/lmdb/lmdb.hpp>
#include <nano/store/lmdb/version.hpp>

nano::store::lmdb::version::version (rsnano::LmdbVersionStoreHandle * handle_a) :
	handle{ handle_a }
{
}

nano::store::lmdb::version::~version ()
{
	if (handle != nullptr)
		rsnano::rsn_lmdb_version_store_destroy (handle);
}

void nano::store::lmdb::version::put (nano::store::write_transaction const & transaction_a, int version)
{
	rsnano::rsn_lmdb_version_store_put (handle, transaction_a.get_rust_handle (), version);
}

int nano::store::lmdb::version::get (nano::store::transaction const & transaction_a) const
{
	return rsnano::rsn_lmdb_version_store_get (handle, transaction_a.get_rust_handle ());
}
