#pragma once

#include <nano/boost/asio/deadline_timer.hpp>
#include <nano/boost/asio/executor_work_guard.hpp>
#include <nano/boost/asio/io_context.hpp>
#include <nano/boost/asio/steady_timer.hpp>
#include <nano/boost/asio/thread_pool.hpp>
#include <nano/lib/relaxed_atomic.hpp>
#include <nano/lib/thread_roles.hpp>
#include <nano/lib/utility.hpp>

#include <boost/thread/thread.hpp>

#include <latch>
#include <thread>

namespace rsnano
{
class ThreadPoolHandle;
}

namespace nano
{
namespace thread_attributes
{
	boost::thread::attributes get_default ();
}

class thread_runner final
{
public:
	thread_runner (boost::asio::io_context &, unsigned num_threads, nano::thread_role::name thread_role = nano::thread_role::name::io);
	~thread_runner ();

	/** Tells the IO context to stop processing events.*/
	void stop_event_processing ();
	/** Wait for IO threads to complete */
	void join ();

private:
	nano::thread_role::name const role;
	std::vector<boost::thread> threads;
	boost::asio::executor_work_guard<boost::asio::io_context::executor_type> io_guard;

private:
	void run (boost::asio::io_context &);
};

class thread_pool final
{
public:
	explicit thread_pool (unsigned, nano::thread_role::name);
	thread_pool (thread_pool const &) = delete;
	~thread_pool ();

	/** This will run when there is an available thread for execution */
	void push_task (std::function<void ()>);

	/** Run a task at a certain point in time */
	void add_timed_task (std::chrono::steady_clock::time_point const & expiry_time, std::function<void ()> task);

	/** Stops any further pushed tasks from executing */
	void stop ();

	rsnano::ThreadPoolHandle * handle;
};

std::unique_ptr<nano::container_info_component> collect_container_info (thread_pool & thread_pool, std::string const & name);

/**
 * Number of available logical processor cores. Might be overridden by setting `NANO_HARDWARE_CONCURRENCY` environment variable
 */
unsigned int hardware_concurrency ();

/**
 * If thread is joinable joins it, otherwise does nothing
 */
bool join_or_pass (std::thread &);
}
