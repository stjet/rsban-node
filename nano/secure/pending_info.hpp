#pragma once

#include "nano/lib/rsnano.hpp"

#include <nano/lib/epoch.hpp>
#include <nano/lib/numbers.hpp>
#include <nano/lib/stream.hpp>

namespace rsnano
{
class ReceivableIteratorHandle;
class PendingKeyDto;
class PendingInfoDto;
}

namespace nano
{
/**
 * Information on an uncollected send
 * This class captures the data stored in a pending table entry
 */
class pending_info final
{
public:
	pending_info () = default;
	pending_info (nano::account const &, nano::amount const &, nano::epoch);
	pending_info (rsnano::PendingInfoDto const & dto);
	size_t db_size () const;
	bool deserialize (nano::stream &);
	bool operator== (nano::pending_info const &) const;
	nano::account source{}; // the account sending the funds
	nano::amount amount{ 0 }; // amount receivable in this transaction
	nano::epoch epoch{ nano::epoch::epoch_0 }; // epoch of sending block, this info is stored here to make it possible to prune the send block
};

// This class represents the data written into the pending (receivable) database table key
// the receiving account and hash of the send block identify a pending db table entry
class pending_key final
{
public:
	pending_key () = default;
	pending_key (nano::account const &, nano::block_hash const &);
	pending_key (rsnano::PendingKeyDto const & dto);
	bool deserialize (nano::stream &);
	bool operator== (nano::pending_key const &) const;
	bool operator< (nano::pending_key const &) const;
	nano::account const & key () const;
	nano::account account{}; // receiving account
	nano::block_hash hash{ 0 }; // hash of the send block
};
// This class iterates receivable enttries for an account
class receivable_iterator
{
public:
	receivable_iterator (rsnano::ReceivableIteratorHandle * handle);
	receivable_iterator (receivable_iterator const &) = delete;
	receivable_iterator (receivable_iterator &&) = delete;
	~receivable_iterator ();

	receivable_iterator & operator= (receivable_iterator && other);

	// Advances to the next receivable entry for the same account
	receivable_iterator & operator++ ();
	bool is_end () const;
	std::pair<nano::pending_key, nano::pending_info> const & operator* () const;
	std::pair<nano::pending_key, nano::pending_info> const * operator->() const;

private:
	void load_next ();

	rsnano::ReceivableIteratorHandle * handle;
	std::optional<std::pair<nano::pending_key, nano::pending_info>> current;
};
} // namespace nano

namespace std
{
template <>
struct hash<::nano::pending_key>
{
	size_t operator() (::nano::pending_key const & data_a) const
	{
		return hash<::nano::uint512_union>{}({ ::nano::uint256_union{ data_a.account.number () }, data_a.hash });
	}
};
}
