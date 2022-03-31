#include <nano/boost/asio/post.hpp>
#include <nano/lib/locks.hpp>
#include <nano/lib/numbers.hpp>
#include <nano/node/signatures.hpp>

rsnano::SignatureCheckSetDto to_check_set_dto (nano::signature_check_set const & check_a)
{
	rsnano::SignatureCheckSetDto check_dto;
	check_dto.messages = check_a.messages;
	check_dto.message_lengths = check_a.message_lengths;
	check_dto.pub_keys = check_a.pub_keys;
	check_dto.signatures = check_a.signatures;
	check_dto.verifications = check_a.verifications;
	check_dto.size = check_a.size;
	return check_dto;
}

nano::signature_checker::signature_checker (unsigned num_threads) :
	thread_pool (num_threads, nano::thread_role::name::signature_checking),
	handle (rsnano::rsn_signature_checker_create (num_threads))
{
}

nano::signature_checker::~signature_checker ()
{
	rsnano::rsn_signature_checker_destroy (handle);
	stop ();
}

std::size_t nano::signature_checker::get_batch_size ()
{
	return rsnano::rsn_signature_checker_batch_size ();
}

void nano::signature_checker::verify (nano::signature_check_set & check_a)
{
	// Don't process anything else if we have stopped
	if (stopped)
	{
		return;
	}

	rsnano::SignatureCheckSetDto check_set_dto{ to_check_set_dto (check_a) };
	if (!rsnano::rsn_signature_checker_verify (handle, &check_set_dto))
	{
		return;
	}

	// Split up the tasks equally over the calling thread and the thread pool.
	// Any overflow on the modulus of the batch_size is given to the calling thread, so the thread pool
	// only ever operates on batch_size sizes.
	std::size_t overflow_size = check_a.size % get_batch_size ();
	std::size_t num_full_batches = check_a.size / get_batch_size ();

	auto const num_threads = thread_pool.get_num_threads ();
	auto total_threads_to_split_over = num_threads + 1;
	auto num_base_batches_each = num_full_batches / total_threads_to_split_over;
	auto num_full_overflow_batches = num_full_batches % total_threads_to_split_over;

	auto size_calling_thread = (num_base_batches_each * get_batch_size ()) + overflow_size;
	auto num_full_batches_thread = (num_base_batches_each * num_threads);
	if (num_full_overflow_batches > 0)
	{
		if (overflow_size == 0)
		{
			// Give the calling thread priority over any batches when there is no excess remainder.
			size_calling_thread += get_batch_size ();
			num_full_batches_thread += num_full_overflow_batches - 1;
		}
		else
		{
			num_full_batches_thread += num_full_overflow_batches;
		}
	}

	release_assert (check_a.size == (num_full_batches_thread * get_batch_size () + size_calling_thread));

	std::promise<void> promise;
	std::future<void> future = promise.get_future ();

	// Verify a number of signature batches over the thread pool (does not block)
	verify_async (check_a, num_full_batches_thread, promise);

	// Verify the rest on the calling thread, this operates on the signatures at the end of the check set
	auto result = verify_batch (check_a, check_a.size - size_calling_thread, size_calling_thread);
	release_assert (result);

	// Blocks until all the work is done
	future.wait ();
}

void nano::signature_checker::stop ()
{
	if (!stopped.exchange (true))
	{
		thread_pool.stop ();
	}
}

void nano::signature_checker::flush ()
{
	while (!stopped && tasks_remaining != 0)
		;
}

bool nano::signature_checker::verify_batch (nano::signature_check_set const & check_a, std::size_t start_index, std::size_t size)
{
	rsnano::SignatureCheckSetDto check_dto{ to_check_set_dto (check_a) };
	return rsnano::rsn_signature_checker_verify_batch (handle, &check_dto, start_index, size);
}

/* This operates on a number of signatures of size (num_batches * batch_size) from the beginning of the check_a pointers.
 * Caller should check the value of the promise which indicates when the work has been completed.
 */
void nano::signature_checker::verify_async (nano::signature_check_set & check_a, std::size_t num_batches, std::promise<void> & promise)
{
	auto task = std::make_shared<Task> (check_a, num_batches);
	++tasks_remaining;

	for (std::size_t batch = 0; batch < num_batches; ++batch)
	{
		auto size = get_batch_size ();
		auto start_index = batch * get_batch_size ();

		thread_pool.push_task ([this, task, size, start_index, &promise] {
			auto result = this->verify_batch (task->check, start_index, size);
			release_assert (result);

			if (--task->pending == 0)
			{
				--tasks_remaining;
				promise.set_value ();
			}
		});
	}
}

bool nano::signature_checker::single_threaded () const
{
	return thread_pool.get_num_threads () == 0;
}
