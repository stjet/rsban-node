#include <nano/lib/thread_roles.hpp>
#include <nano/lib/rsnano.hpp>
#include <nano/node/confirming_set.hpp>
#include <nano/node/write_database_queue.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/store/component.hpp>

nano::confirming_set::confirming_set (nano::ledger & ledger, nano::write_database_queue & write_queue, std::chrono::milliseconds batch_time) :
	handle{rsnano::rsn_confirming_set_create(ledger.handle, write_queue.handle, batch_time.count())}
{
}

nano::confirming_set::~confirming_set ()
{
	rsnano::rsn_confirming_set_destroy(handle);
}

void nano::confirming_set::add (nano::block_hash const & hash)
{
	rsnano::rsn_confirming_set_add(handle, hash.bytes.data());
}

void nano::confirming_set::start ()
{
	rsnano::rsn_confirming_set_start(handle);
}

void nano::confirming_set::stop ()
{
	rsnano::rsn_confirming_set_stop(handle);
}

bool nano::confirming_set::exists (nano::block_hash const & hash) const
{
	return rsnano::rsn_confirming_set_exists(handle, hash.bytes.data());
}

std::size_t nano::confirming_set::size () const
{
	return rsnano::rsn_confirming_set_len(handle);
}

std::unique_ptr<nano::container_info_component> nano::confirming_set::collect_container_info (std::string const & name) const
{
	auto info_handle = rsnano::rsn_confirming_set_collect_container_info (handle, name.c_str ());
	return std::make_unique<nano::container_info_composite> (info_handle);
}
