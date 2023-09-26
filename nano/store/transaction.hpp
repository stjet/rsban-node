#pragma once

#include <nano/store/tables.hpp>

#include <memory>

namespace rsnano
{
	class TransactionHandle;
}

namespace nano
{
class transaction_impl
{
public:
	virtual ~transaction_impl () = default;
	virtual void * get_handle () const = 0;
};

class read_transaction_impl : public transaction_impl
{
public:
	virtual void reset () = 0;
	virtual void renew () = 0;
};

class write_transaction_impl : public transaction_impl
{
public:
	virtual void commit () = 0;
	virtual void renew () = 0;
	virtual bool contains (nano::tables table_a) const = 0;
};

class transaction
{
public:
	virtual ~transaction () = default;
	virtual rsnano::TransactionHandle * get_rust_handle () const = 0;
};

class transaction_wrapper : public transaction
{
public:
	transaction_wrapper (rsnano::TransactionHandle * handle_a) :
		handle{ handle_a }
	{
	}
	rsnano::TransactionHandle * get_rust_handle () const override
	{
		return handle;
	}

private:
	rsnano::TransactionHandle * handle;
};

/**
 * RAII wrapper of a read MDB_txn where the constructor starts the transaction
 * and the destructor aborts it.
 */
class read_transaction : public transaction
{
public:
	virtual void reset () = 0;
	virtual void renew () = 0;
	virtual void refresh () = 0;
};

/**
 * RAII wrapper of a read-write MDB_txn where the constructor starts the transaction
 * and the destructor commits it.
 */
class write_transaction : public transaction
{
public:
	virtual void commit () = 0;
	virtual void renew () = 0;
	virtual void refresh () = 0;
	virtual bool contains (nano::tables table_a) const = 0;
};
} // namespace nano
