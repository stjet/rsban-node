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

