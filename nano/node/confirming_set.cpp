#include <nano/lib/rsnano.hpp>
#include <nano/lib/rsnanoutils.hpp>
#include <nano/lib/thread_roles.hpp>
#include <nano/node/confirming_set.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/store/component.hpp>

namespace
{
void block_callback (void * context_a, rsnano::BlockHandle * block_handle)
{
	auto callback = static_cast<std::function<void (std::shared_ptr<nano::block> const &)> *> (context_a);
	auto block{ nano::block_handle_to_block (rsnano::rsn_block_clone (block_handle)) };
	(*callback) (block);
}

void delete_block_callback_context (void * context_a)
{
	auto callback = static_cast<std::function<void (std::shared_ptr<nano::block> const &)> *> (context_a);
	delete callback;
}

void block_hash_callback (void * context_a, const uint8_t * hash_bytes)
{
	auto callback = static_cast<std::function<void (nano::block_hash const &)> *> (context_a);
	auto hash{ nano::block_hash::from_bytes (hash_bytes) };
	(*callback) (hash);
}

void delete_block_hash_callback_context (void * context_a)
{
	auto callback = static_cast<std::function<void (nano::block_hash const &)> *> (context_a);
	delete callback;
}
}

nano::confirming_set::confirming_set (rsnano::ConfirmingSetHandle * handle) :
	handle{ handle }
{
}

nano::confirming_set::confirming_set (nano::ledger & ledger, std::chrono::milliseconds batch_time) :
	handle{ rsnano::rsn_confirming_set_create (ledger.handle, batch_time.count ()) }
{
}

nano::confirming_set::~confirming_set ()
{
	rsnano::rsn_confirming_set_destroy (handle);
}

void nano::confirming_set::add (nano::block_hash const & hash)
{
	rsnano::rsn_confirming_set_add (handle, hash.bytes.data ());
}

void nano::confirming_set::start ()
{
	rsnano::rsn_confirming_set_start (handle);
}

void nano::confirming_set::stop ()
{
	rsnano::rsn_confirming_set_stop (handle);
}

bool nano::confirming_set::exists (nano::block_hash const & hash) const
{
	return rsnano::rsn_confirming_set_exists (handle, hash.bytes.data ());
}

std::size_t nano::confirming_set::size () const
{
	return rsnano::rsn_confirming_set_len (handle);
}

void nano::confirming_set::add_cemented_observer (std::function<void (std::shared_ptr<nano::block> const &)> const & callback_a)
{
	auto context = new std::function<void (std::shared_ptr<nano::block> const &)> (callback_a);
	rsnano::rsn_confirming_set_add_cemented_observer (handle, block_callback, context, delete_block_callback_context);
}

void nano::confirming_set::add_block_already_cemented_observer (std::function<void (nano::block_hash const &)> const & callback_a)
{
	auto context = new std::function<void (nano::block_hash const &)> (callback_a);
	rsnano::rsn_confirming_set_add_already_cemented_observer (handle, block_hash_callback, context, delete_block_hash_callback_context);
}
