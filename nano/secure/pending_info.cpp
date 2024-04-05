#include <nano/lib/rsnano.hpp>
#include <nano/secure/pending_info.hpp>

#include <optional>
#include <utility>

nano::pending_info::pending_info (nano::account const & source_a, nano::amount const & amount_a, nano::epoch epoch_a) :
	source (source_a),
	amount (amount_a),
	epoch (epoch_a)
{
}

nano::pending_info::pending_info (rsnano::PendingInfoDto const & dto) :
	source{ nano::account::from_bytes (dto.source) },
	amount (nano::amount::from_bytes (dto.amount)),
	epoch (static_cast<nano::epoch> (dto.epoch))
{
}

bool nano::pending_info::deserialize (nano::stream & stream_a)
{
	auto error (false);
	try
	{
		nano::read (stream_a, source.bytes);
		nano::read (stream_a, amount.bytes);
		nano::read (stream_a, epoch);
	}
	catch (std::runtime_error const &)
	{
		error = true;
	}

	return error;
}

size_t nano::pending_info::db_size () const
{
	return sizeof (source) + sizeof (amount) + sizeof (epoch);
}

bool nano::pending_info::operator== (nano::pending_info const & other_a) const
{
	return source == other_a.source && amount == other_a.amount && epoch == other_a.epoch;
}

nano::pending_key::pending_key (nano::account const & account_a, nano::block_hash const & hash_a) :
	account (account_a),
	hash (hash_a)
{
}

nano::pending_key::pending_key (rsnano::PendingKeyDto const & dto) :
	account (nano::account::from_bytes (dto.account)),
	hash (nano::block_hash::from_bytes (dto.hash))
{
}

bool nano::pending_key::deserialize (nano::stream & stream_a)
{
	auto error (false);
	try
	{
		nano::read (stream_a, account.bytes);
		nano::read (stream_a, hash.bytes);
	}
	catch (std::runtime_error const &)
	{
		error = true;
	}

	return error;
}

bool nano::pending_key::operator== (nano::pending_key const & other_a) const
{
	return account == other_a.account && hash == other_a.hash;
}

nano::account const & nano::pending_key::key () const
{
	return account;
}

bool nano::pending_key::operator< (nano::pending_key const & other_a) const
{
	return account == other_a.account ? hash < other_a.hash : account < other_a.account;
}

nano::receivable_iterator::receivable_iterator (rsnano::ReceivableIteratorHandle * handle) :
	handle{ handle },
	current{}
{
	load_next ();
}

nano::receivable_iterator::~receivable_iterator ()
{
	if (handle != nullptr)
	{
		rsnano::rsn_receivable_iterator_destroy (handle);
		handle = nullptr;
	}
}

nano::receivable_iterator & nano::receivable_iterator::operator= (nano::receivable_iterator && other)
{
	if (handle != nullptr)
	{
		rsnano::rsn_receivable_iterator_destroy (handle);
	}
	handle = other.handle;
	current = other.current;
	other.handle = nullptr;
	return *this;
}

nano::receivable_iterator & nano::receivable_iterator::operator++ ()
{
	load_next ();
	return *this;
}

bool nano::receivable_iterator::is_end () const
{
	return !current.has_value ();
}

std::pair<nano::pending_key, nano::pending_info> const & nano::receivable_iterator::operator* () const
{
	return current.value ();
}

std::pair<nano::pending_key, nano::pending_info> const * nano::receivable_iterator::operator->() const
{
	return &current.value ();
}

void nano::receivable_iterator::load_next ()
{
	rsnano::PendingKeyDto key_dto;
	rsnano::PendingInfoDto info_dto;
	if (rsnano::rsn_receivable_iterator_next (handle, &key_dto, &info_dto))
	{
		current = std::make_pair (nano::pending_key{ key_dto }, nano::pending_info{ info_dto });
	}
	else
	{
		current = std::nullopt;
	}
}
