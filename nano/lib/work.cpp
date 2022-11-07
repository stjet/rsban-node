#include <nano/crypto_lib/random_pool.hpp>
#include <nano/lib/blocks.hpp>
#include <nano/lib/epoch.hpp>
#include <nano/lib/threading.hpp>
#include <nano/lib/work.hpp>
#include <nano/node/xorshift.hpp>

#include <future>
#include <thread>

nano::work_ticket::work_ticket () :
	handle{ rsnano::rsn_work_ticket_create () }
{
}
nano::work_ticket::work_ticket (rsnano::WorkTicketHandle * handle_a) :
	handle{ handle_a }
{
}

nano::work_ticket::work_ticket (nano::work_ticket const & other_a) :
	handle{ rsnano::rsn_work_ticket_clone (other_a.handle) }
{
}

nano::work_ticket::work_ticket (nano::work_ticket && other_a) :
	handle{ other_a.handle }
{
	other_a.handle = nullptr;
}
nano::work_ticket::~work_ticket ()
{
	if (handle != nullptr)
		rsnano::rsn_work_ticket_destroy (handle);
}

bool nano::work_ticket::expired () const
{
	return rsnano::rsn_work_ticket_expired (handle);
}

std::string nano::to_string (nano::work_version const version_a)
{
	std::string result ("invalid");
	switch (version_a)
	{
		case nano::work_version::work_1:
			result = "work_1";
			break;
		case nano::work_version::unspecified:
			result = "unspecified";
			break;
	}
	return result;
}

nano::work_pool::work_pool (nano::network_constants & network_constants, unsigned max_threads_a, std::chrono::nanoseconds pow_rate_limiter_a, std::function<boost::optional<uint64_t> (nano::work_version const, nano::root const, uint64_t, nano::work_ticket)> opencl_a) :
	network_constants{ network_constants },
	done (false),
	pow_rate_limiter (pow_rate_limiter_a),
	opencl (opencl_a)
{
	auto nw_constants_dto = network_constants.to_dto ();
	handle = rsnano::rsn_work_pool_create (&nw_constants_dto, max_threads_a, pow_rate_limiter_a.count ());
	static_assert (ATOMIC_INT_LOCK_FREE == 2, "Atomic int needed");
	boost::thread::attributes attrs;
	nano::thread_attributes::set (attrs);
	auto count (network_constants.is_dev_network () ? std::min (max_threads_a, 1u) : std::min (max_threads_a, std::max (1u, nano::hardware_concurrency ())));
	if (opencl)
	{
		// One thread to handle OpenCL
		++count;
	}
	for (auto i (0u); i < count; ++i)
	{
		threads.emplace_back (attrs, [this, i] () {
			nano::thread_role::set (nano::thread_role::name::work);
			nano::work_thread_reprioritize ();
			loop (i);
		});
	}
}

nano::work_pool::~work_pool ()
{
	stop ();
	for (auto & i : threads)
	{
		i.join ();
	}
	rsnano::rsn_work_pool_destroy (handle);
}

class blake2b_wrapper
{
public:
	blake2b_wrapper (size_t size) :
		handle{ rsnano::rsn_blake2b_create (size) }
	{
	}
	~blake2b_wrapper ()
	{
		rsnano::rsn_blake2b_destroy (handle);
	}
	void update (const uint8_t * data, size_t size)
	{
		rsnano::rsn_blake2b_update (handle, data, size);
	}
	void final (uint8_t * output, size_t size)
	{
		rsnano::rsn_blake2b_final (handle, output, size);
	}

private:
	rsnano::Blake2bHandle * handle;
};

void nano::work_pool::loop (uint64_t thread)
{
	// Quick RNG for work attempts.
	xorshift1024star rng;
	nano::random_pool::generate_block (reinterpret_cast<uint8_t *> (rng.s.data ()), rng.s.size () * sizeof (decltype (rng.s)::value_type));
	uint64_t work;
	uint64_t output;
	blake2b_wrapper hash (sizeof (output));
	nano::unique_lock<nano::mutex> lock (mutex);
	auto pow_sleep = pow_rate_limiter;
	while (!done)
	{
		auto empty (pending.empty ());
		if (thread == 0)
		{
			// Only work thread 0 notifies work observers
			work_observers.notify (!empty);
		}
		if (!empty)
		{
			auto current_l (pending.front ());
			auto ticket_l{ create_work_ticket () };
			lock.unlock ();
			output = 0;
			boost::optional<uint64_t> opt_work;
			if (thread == 0 && opencl)
			{
				opt_work = opencl (current_l.version, current_l.item, current_l.difficulty, ticket_l);
			}
			if (opt_work.is_initialized ())
			{
				work = *opt_work;
				output = network_constants.work.value (current_l.item, work);
			}
			else
			{
				// ticket != ticket_l indicates a different thread found a solution and we should stop
				while (!ticket_l.expired () && output < current_l.difficulty)
				{
					// Don't query main memory every iteration in order to reduce memory bus traffic
					// All operations here operate on stack memory
					// Count iterations down to zero since comparing to zero is easier than comparing to another number
					unsigned iteration (256);
					while (iteration && output < current_l.difficulty)
					{
						work = rng.next ();
						hash.update (reinterpret_cast<uint8_t *> (&work), sizeof (work));
						hash.update (current_l.item.bytes.data (), current_l.item.bytes.size ());
						hash.final (reinterpret_cast<uint8_t *> (&output), sizeof (output));
						iteration -= 1;
					}

					// Add a rate limiter (if specified) to the pow calculation to save some CPUs which don't want to operate at full throttle
					if (pow_sleep != std::chrono::nanoseconds (0))
					{
						std::this_thread::sleep_for (pow_sleep);
					}
				}
			}
			lock.lock ();
			if (!ticket_l.expired ())
			{
				// If the ticket matches what we started with, we're the ones that found the solution
				debug_assert (output >= current_l.difficulty);
				debug_assert (current_l.difficulty == 0 || network_constants.work.value (current_l.item, work) == output);
				// Signal other threads to stop their work next time they check ticket
				expire_work_tickets ();
				pending.pop_front ();
				lock.unlock ();
				current_l.callback (work);
				lock.lock ();
			}
			else
			{
				// A different thread found a solution
			}
		}
		else
		{
			// Wait for a work request
			producer_condition.wait (lock);
		}
	}
}

void nano::work_pool::cancel (nano::root const & root_a)
{
	nano::lock_guard<nano::mutex> lock (mutex);
	if (!done)
	{
		if (!pending.empty ())
		{
			if (pending.front ().item == root_a)
			{
				expire_work_tickets ();
			}
		}
		pending.remove_if ([&root_a] (decltype (pending)::value_type const & item_a) {
			bool result{ false };
			if (item_a.item == root_a)
			{
				if (item_a.callback)
				{
					item_a.callback (boost::none);
				}
				result = true;
			}
			return result;
		});
	}
}

void nano::work_pool::stop ()
{
	{
		nano::lock_guard<nano::mutex> lock (mutex);
		done = true;
		expire_work_tickets ();
	}
	producer_condition.notify_all ();
}

void nano::work_pool::generate (nano::work_version const version_a, nano::root const & root_a, uint64_t difficulty_a, std::function<void (boost::optional<uint64_t> const &)> callback_a)
{
	debug_assert (!root_a.is_zero ());
	if (!threads.empty ())
	{
		{
			nano::lock_guard<nano::mutex> lock (mutex);
			pending.emplace_back (version_a, root_a, difficulty_a, callback_a);
		}
		producer_condition.notify_all ();
	}
	else if (callback_a)
	{
		callback_a (boost::none);
	}
}

boost::optional<uint64_t> nano::work_pool::generate (nano::root const & root_a)
{
	debug_assert (network_constants.is_dev_network ());
	return generate (nano::work_version::work_1, root_a, network_constants.work.get_base ());
}

boost::optional<uint64_t> nano::work_pool::generate (nano::root const & root_a, uint64_t difficulty_a)
{
	debug_assert (network_constants.is_dev_network ());
	return generate (nano::work_version::work_1, root_a, difficulty_a);
}

boost::optional<uint64_t> nano::work_pool::generate (nano::work_version const version_a, nano::root const & root_a, uint64_t difficulty_a)
{
	boost::optional<uint64_t> result;
	if (!threads.empty ())
	{
		std::promise<boost::optional<uint64_t>> work;
		std::future<boost::optional<uint64_t>> future = work.get_future ();
		generate (version_a, root_a, difficulty_a, [&work] (boost::optional<uint64_t> work_a) {
			work.set_value (work_a);
		});
		result = future.get ().value ();
	}
	return result;
}

size_t nano::work_pool::size ()
{
	nano::lock_guard<nano::mutex> lock (mutex);
	return pending.size ();
}

size_t nano::work_pool::pending_size ()
{
	nano::lock_guard<nano::mutex> guard (mutex);
	return pending.size ();
}

std::unique_ptr<nano::container_info_component> nano::work_pool::collect_observer_info ()
{
	return work_observers.collect_container_info ("work_observers");
}

size_t nano::work_pool::pending_value_size () const
{
	return sizeof (decltype (pending)::value_type);
}

size_t nano::work_pool::thread_count () const
{
	return threads.size ();
}

bool nano::work_pool::has_opencl () const
{
	return opencl != nullptr;
}
uint64_t nano::work_pool::threshold_base (const nano::work_version version_a) const
{
	return network_constants.work.threshold_base (version_a);
}
uint64_t nano::work_pool::difficulty (const nano::work_version version_a, const nano::root & root_a, const uint64_t work_a) const
{
	return network_constants.work.difficulty (version_a, root_a, work_a);
}
nano::work_ticket nano::work_pool::create_work_ticket ()
{
	return nano::work_ticket (rsnano::rsn_work_pool_create_work_ticket (handle));
}
void nano::work_pool::expire_work_tickets ()
{
	rsnano::rsn_work_pool_expire_work_tickets (handle);
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (work_pool & work_pool, std::string const & name)
{
	size_t count = work_pool.pending_size ();
	auto sizeof_element = work_pool.pending_value_size ();
	auto composite = std::make_unique<container_info_composite> (name);
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "pending", count, sizeof_element }));
	composite->add_component (work_pool.collect_observer_info ());
	return composite;
}
