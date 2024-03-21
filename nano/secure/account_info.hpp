#pragma once

#include <nano/lib/epoch.hpp>
#include <nano/lib/numbers.hpp>
#include <nano/lib/stream.hpp>
#include <nano/lib/timer.hpp>

namespace rsnano
{
class AccountInfoHandle;
}

namespace nano
{
/**
 * Latest information about an account
 */
class account_info final
{
public:
	account_info ();
	account_info (rsnano::AccountInfoHandle * handle_a);
	account_info (nano::block_hash const &, nano::account const &, nano::block_hash const &, nano::amount const &, nano::seconds_t modified, uint64_t, epoch);
	account_info (account_info const &);
	account_info (account_info &&);
	~account_info ();
	account_info & operator= (account_info const &);
	bool deserialize (nano::stream &);
	bool operator== (nano::account_info const &) const;
	bool operator!= (nano::account_info const &) const;
	size_t db_size () const;
	nano::epoch epoch () const;
	nano::block_hash head () const;
	nano::account representative () const;
	nano::block_hash open_block () const;
	nano::amount balance () const;
	nano::seconds_t modified () const;
	uint64_t block_count () const;
	rsnano::AccountInfoHandle * handle;
};
}
