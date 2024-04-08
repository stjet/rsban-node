#pragma once

#include <nano/lib/locks.hpp>

namespace rsnano
{
class WriteQueueHandle;
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

/**
 * Allocates database write access in a fair maner rather than directly waiting for mutex aquisition
 * Users should wait() for access to database write transaction and hold the write_guard until complete
 */
class write_queue final
{
public:
	write_queue (bool use_noops_a);
	write_queue (write_queue const &) = delete;
	write_queue (write_queue &&) = delete;
	~write_queue ();
	/** Blocks until we are at the head of the queue and blocks other waiters until write_guard goes out of scope */
	[[nodiscard ("write_guard blocks other waiters")]] write_guard wait (nano::store::writer writer);

	/** Returns true if this writer is now at the front of the queue */
	bool process (nano::store::writer writer);

	/** Returns true if this writer is anywhere in the queue. Currently only used in tests */
	bool contains (nano::store::writer writer);

	/** Doesn't actually pop anything until the returned write_guard is out of scope */
	write_guard pop ();

	rsnano::WriteQueueHandle * handle;
};
}
