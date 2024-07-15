#pragma once

#include <nano/lib/locks.hpp>

namespace rsnano
{
class WriteGuardHandle;
}

namespace nano::store
{
/** Distinct areas write locking is done, order is irrelevant */
enum class writer
{
	confirmation_height,
	process_batch,
	pruning,
	voting_final,
	testing // Used in tests to emulate a write lock
};

class write_guard final
{
public:
	write_guard (rsnano::WriteGuardHandle * handle_a);
	~write_guard ();
	write_guard (write_guard const &) = delete;
	write_guard & operator= (write_guard const &) = delete;
	write_guard (write_guard &&) noexcept;
	write_guard & operator= (write_guard &&) noexcept;
	void release ();
	bool is_owned () const;

	rsnano::WriteGuardHandle * handle;

private:
	bool owns{ true };
};
}
