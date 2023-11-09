#pragma once

#include <nano/lib/diagnosticsconfig.hpp>
#include <nano/lib/timer.hpp>
#include <nano/store/component.hpp>
#include <nano/store/transaction.hpp>

#include <boost/property_tree/ptree_fwd.hpp>
#include <boost/stacktrace/stacktrace_fwd.hpp>

namespace nano::store::lmdb
{
class txn_callbacks
{
public:
	// takes a txn_id and is_write
	std::function<void (uint64_t, bool)> txn_start{ [] (uint64_t, bool) {} };

	// takes a txn_id
	std::function<void (uint64_t)> txn_end{ [] (uint64_t) {} };
};

class read_transaction_impl final : public nano::store::read_transaction
{
public:
	read_transaction_impl (read_transaction_impl const &) = delete;
	read_transaction_impl (read_transaction_impl &&) = delete;
	read_transaction_impl (rsnano::TransactionHandle * handle_a);
	~read_transaction_impl () override;
	void reset () override;
	void renew () override;
	void refresh () override;
	void refresh_if_needed (std::chrono::milliseconds max_age) const override;
	rsnano::TransactionHandle * get_rust_handle () const override
	{
		return txn_handle;
	};

	rsnano::TransactionHandle * txn_handle;
};

class write_transaction_impl final : public nano::store::write_transaction
{
public:
	write_transaction_impl (write_transaction_impl const &) = delete;
	write_transaction_impl (write_transaction_impl &&) = delete;
	write_transaction_impl (rsnano::TransactionHandle * handle_a);
	~write_transaction_impl () override;
	void commit () override;
	void renew () override;
	void refresh () override;
	void refresh_if_needed (std::chrono::milliseconds max_age) override;
	bool contains (nano::tables table_a) const override;
	rsnano::TransactionHandle * get_rust_handle () const override
	{
		return txn_handle;
	};

	rsnano::TransactionHandle * txn_handle;
};
} // namespace nano
