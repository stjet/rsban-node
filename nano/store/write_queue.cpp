#include <nano/lib/rsnano.hpp>
#include <nano/store/write_queue.hpp>

nano::store::write_guard::write_guard (rsnano::WriteGuardHandle * handle_a) :
	handle{ handle_a }
{
}

nano::store::write_guard::write_guard (nano::store::write_guard && write_guard_a) noexcept :
	handle (write_guard_a.handle),
	owns (write_guard_a.owns)
{
	write_guard_a.owns = false;
	write_guard_a.handle = nullptr;
}

nano::store::write_guard & nano::store::write_guard::operator= (nano::store::write_guard && write_guard_a) noexcept
{
	if (owns && handle != nullptr)
		rsnano::rsn_write_guard_destroy (handle);

	owns = write_guard_a.owns;
	handle = write_guard_a.handle;

	write_guard_a.owns = false;
	write_guard_a.handle = nullptr;
	return *this;
}

nano::store::write_guard::~write_guard ()
{
	if (owns)
	{
		rsnano::rsn_write_guard_destroy (handle);
	}
}

bool nano::store::write_guard::is_owned () const
{
	return owns;
}

void nano::store::write_guard::release ()
{
	if (owns)
	{
		rsnano::rsn_write_guard_release (handle);
	}
	owns = false;
}

nano::store::write_queue::write_queue (bool use_noops_a) :
	handle{ rsnano::rsn_write_database_queue_create (use_noops_a) }
{
}

nano::store::write_queue::~write_queue ()
{
	rsnano::rsn_write_database_queue_destroy (handle);
}

nano::store::write_guard nano::store::write_queue::wait (nano::store::writer writer)
{
	auto guard_handle = rsnano::rsn_write_database_queue_wait (handle, static_cast<uint8_t> (writer));
	return nano::store::write_guard (guard_handle);
}

bool nano::store::write_queue::contains (nano::store::writer writer)
{
	return rsnano::rsn_write_database_queue_contains (handle, static_cast<uint8_t> (writer));
}

bool nano::store::write_queue::process (nano::store::writer writer)
{
	return rsnano::rsn_write_database_queue_process (handle, static_cast<uint8_t> (writer));
}

nano::store::write_guard nano::store::write_queue::pop ()
{
	return nano::store::write_guard (rsnano::rsn_write_database_queue_pop (handle));
}
