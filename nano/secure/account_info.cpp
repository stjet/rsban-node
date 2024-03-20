#include <nano/secure/account_info.hpp>
#include <nano/lib/rsnano.hpp>

nano::account_info::account_info () :
	account_info (0, 0, 0, 0, 0, 0, nano::epoch::epoch_0)
{
}

nano::account_info::account_info (rsnano::AccountInfoHandle * handle_a) :
	handle{ handle_a }
{
}

nano::account_info::account_info (nano::block_hash const & head_a, nano::account const & representative_a, nano::block_hash const & open_block_a, nano::amount const & balance_a, nano::seconds_t modified_a, uint64_t block_count_a, nano::epoch epoch_a) :
	handle{ rsnano::rsn_account_info_create (head_a.bytes.data (), representative_a.bytes.data (), open_block_a.bytes.data (), balance_a.bytes.data (), modified_a, block_count_a, static_cast<uint8_t> (epoch_a)) }
{
}

nano::account_info::account_info (nano::account_info const & other_a) :
	handle{ rsnano::rsn_account_info_clone (other_a.handle) }
{
}

nano::account_info::account_info (nano::account_info && other_a) :
	handle{ other_a.handle }
{
	other_a.handle = nullptr;
}

nano::account_info::~account_info ()
{
	if (handle)
		rsnano::rsn_account_info_destroy (handle);
}

nano::account_info & nano::account_info::operator= (nano::account_info const & other_a)
{
	if (handle)
		rsnano::rsn_account_info_destroy (handle);
	handle = rsnano::rsn_account_info_clone (other_a.handle);
	return *this;
}

bool nano::account_info::deserialize (nano::stream & stream_a)
{
	bool success = rsnano::rsn_account_info_deserialize (handle, &stream_a);
	return !success;
}

bool nano::account_info::operator== (nano::account_info const & other_a) const
{
	return rsnano::rsn_account_info_equals (handle, other_a.handle);
}

bool nano::account_info::operator!= (nano::account_info const & other_a) const
{
	return !(*this == other_a);
}

size_t nano::account_info::db_size () const
{
	return rsnano::rsn_account_info_db_size ();
}

nano::epoch nano::account_info::epoch () const
{
	rsnano::AccountInfoDto dto;
	rsnano::rsn_account_info_values (handle, &dto);
	return static_cast<nano::epoch> (dto.epoch);
}
nano::block_hash nano::account_info::head () const
{
	rsnano::AccountInfoDto dto;
	rsnano::rsn_account_info_values (handle, &dto);
	nano::block_hash head;
	std::copy (std::begin (dto.head), std::end (dto.head), std::begin (head.bytes));
	return head;
}

nano::account nano::account_info::representative () const
{
	rsnano::AccountInfoDto dto;
	rsnano::rsn_account_info_values (handle, &dto);
	nano::account representative;
	std::copy (std::begin (dto.representative), std::end (dto.representative), std::begin (representative.bytes));
	return representative;
}

nano::block_hash nano::account_info::open_block () const
{
	rsnano::AccountInfoDto dto;
	rsnano::rsn_account_info_values (handle, &dto);
	nano::block_hash open_block;
	std::copy (std::begin (dto.open_block), std::end (dto.open_block), std::begin (open_block.bytes));
	return open_block;
}
nano::amount nano::account_info::balance () const
{
	rsnano::AccountInfoDto dto;
	rsnano::rsn_account_info_values (handle, &dto);
	nano::amount balance;
	std::copy (std::begin (dto.balance), std::end (dto.balance), std::begin (balance.bytes));
	return balance;
}
uint64_t nano::account_info::modified () const
{
	rsnano::AccountInfoDto dto;
	rsnano::rsn_account_info_values (handle, &dto);
	return dto.modified;
}
uint64_t nano::account_info::block_count () const
{
	rsnano::AccountInfoDto dto;
	rsnano::rsn_account_info_values (handle, &dto);
	return dto.block_count;
}

