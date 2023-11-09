#include <nano/lib/jsonconfig.hpp>
#include <nano/lib/logger_mt.hpp>
#include <nano/lib/rsnano.hpp>
#include <nano/lib/thread_roles.hpp>
#include <nano/lib/utility.hpp>
#include <nano/store/component.hpp>
#include <nano/store/lmdb/lmdb_env.hpp>
#include <nano/store/lmdb/transaction_impl.hpp>

#include <boost/format.hpp>

#ifdef _WIN32
#ifndef NOMINMAX
#define NOMINMAX
#endif
#endif
#include <boost/stacktrace.hpp>

nano::store::lmdb::read_transaction_impl::read_transaction_impl (rsnano::TransactionHandle * handle_a) :
	txn_handle{ handle_a }
{
}

nano::store::lmdb::read_transaction_impl::~read_transaction_impl ()
{
	rsnano::rsn_lmdb_read_txn_destroy (txn_handle);
}

void nano::store::lmdb::read_transaction_impl::reset ()
{
	rsnano::rsn_lmdb_read_txn_reset (txn_handle);
}

void nano::store::lmdb::read_transaction_impl::renew ()
{
	rsnano::rsn_lmdb_read_txn_renew (txn_handle);
}

void nano::store::lmdb::read_transaction_impl::refresh ()
{
	rsnano::rsn_lmdb_read_txn_refresh (txn_handle);
}

void nano::store::lmdb::read_transaction_impl::refresh_if_needed (std::chrono::milliseconds max_age) const
{
	rsnano::rsn_lmdb_read_txn_refresh_if_needed (txn_handle, max_age.count ());
}

nano::store::lmdb::write_transaction_impl::write_transaction_impl (rsnano::TransactionHandle * handle_a) :
	txn_handle{ handle_a }
{
}

nano::store::lmdb::write_transaction_impl::~write_transaction_impl ()
{
	rsnano::rsn_lmdb_write_txn_destroy (txn_handle);
}

void nano::store::lmdb::write_transaction_impl::commit ()
{
	rsnano::rsn_lmdb_write_txn_commit (txn_handle);
}

void nano::store::lmdb::write_transaction_impl::renew ()
{
	rsnano::rsn_lmdb_write_txn_renew (txn_handle);
}

void nano::store::lmdb::write_transaction_impl::refresh ()
{
	rsnano::rsn_lmdb_write_txn_refresh (txn_handle);
}

void nano::store::lmdb::write_transaction_impl::refresh_if_needed (std::chrono::milliseconds max_age)
{
	rsnano::rsn_lmdb_write_txn_refresh_if_needed (txn_handle, max_age.count ());
}

bool nano::store::lmdb::write_transaction_impl::contains (nano::tables table_a) const
{
	// LMDB locks on every write
	return true;
}
