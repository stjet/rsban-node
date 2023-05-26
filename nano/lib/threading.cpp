#include "nano/lib/rsnano.hpp"
#include "nano/lib/thread_roles.hpp"

#include <nano/boost/asio/post.hpp>
#include <nano/lib/config.hpp>
#include <nano/lib/threading.hpp>

#include <boost/format.hpp>

#include <chrono>
#include <exception>
#include <future>
#include <iostream>
#include <stdexcept>
#include <thread>

/*
 * thread_attributes
 */

boost::thread::attributes nano::thread_attributes::get_default ()
{
	boost::thread::attributes attrs;
	attrs.set_stack_size (8000000); // 8MB
	return attrs;
}

/*
 * thread_runner
 */

nano::thread_runner::thread_runner (boost::asio::io_context & io_ctx_a, unsigned num_threads, const nano::thread_role::name thread_role_a) :
	io_guard{ boost::asio::make_work_guard (io_ctx_a) },
	role{ thread_role_a }
{
	for (auto i (0u); i < num_threads; ++i)
	{
		threads.emplace_back (nano::thread_attributes::get_default (), [this, &io_ctx_a] () {
			nano::thread_role::set (role);

			// In a release build, catch and swallow any exceptions,
			// In debug mode let if fall through

#ifndef NDEBUG
			run (io_ctx_a);
#else
			try
			{
				run (io_ctx_a);
			}
			catch (std::exception const & ex)
			{
				std::cerr << ex.what () << std::endl;
			}
			catch (...)
			{
			}
#endif
		});
	}
}

nano::thread_runner::~thread_runner ()
{
	join ();
}

void nano::thread_runner::run (boost::asio::io_context & io_ctx_a)
{
#if NANO_ASIO_HANDLER_TRACKING == 0
	io_ctx_a.run ();
#else
	nano::timer<> timer;
	timer.start ();
	while (true)
	{
		timer.restart ();
		// Run at most 1 completion handler and record the time it took to complete (non-blocking)
		auto count = io_ctx_a.poll_one ();
		if (count == 1 && timer.since_start ().count () >= NANO_ASIO_HANDLER_TRACKING)
		{
			auto timestamp = std::chrono::duration_cast<std::chrono::microseconds> (std::chrono::system_clock::now ().time_since_epoch ()).count ();
			std::cout << (boost::format ("[%1%] io_thread held for %2%ms") % timestamp % timer.since_start ().count ()).str () << std::endl;
		}
		// Sleep for a bit to give more time slices to other threads
		std::this_thread::sleep_for (std::chrono::milliseconds (5));
		std::this_thread::yield ();
	}
#endif
}

void nano::thread_runner::join ()
{
	io_guard.reset ();
	for (auto & i : threads)
	{
		if (i.joinable ())
		{
			i.join ();
		}
	}
}

void nano::thread_runner::stop_event_processing ()
{
	io_guard.get_executor ().context ().stop ();
}

/*
 * thread_pool
 */

nano::thread_pool::thread_pool (unsigned num_threads, nano::thread_role::name thread_name) :
	handle{ rsnano::rsn_thread_pool_create (num_threads, nano::thread_role::get_string (thread_name).c_str ()) }
{
}

nano::thread_pool::~thread_pool ()
{
	rsnano::rsn_thread_pool_destroy (handle);
}

void nano::thread_pool::stop ()
{
	rsnano::rsn_thread_pool_stop (handle);
}

namespace
{
void execute_task (void * context)
{
	auto callback = static_cast<std::function<void ()> *> (context);
	(*callback) ();
}

void delete_task_context (void * context)
{
	auto callback = static_cast<std::function<void ()> *> (context);
	delete callback;
}
}

void nano::thread_pool::push_task (std::function<void ()> task)
{
	auto context = new std::function<void ()> ([task] () {
		try{
			task();
		}
		catch (std::exception e){
			std::cerr << "Thread pool task failed: " << e.what() << std::endl;
		}
		catch (...) {
			std::cerr << "Thread pool task failed!" << std::endl;
		} });
	rsnano::rsn_thread_pool_push_task (handle, execute_task, context, delete_task_context);
}

void nano::thread_pool::add_timed_task (std::chrono::steady_clock::time_point const & expiry_time, std::function<void ()> task)
{
	auto context = new std::function<void ()> ([task] () {
		try{
			task();
		}
		catch (std::exception e){
			std::cerr << "Thread pool task failed: " << e.what() << std::endl;
		}
		catch (...) {
			std::cerr << "Thread pool task failed!" << std::endl;
		} });
	int64_t delay_ms = std::chrono::duration_cast<std::chrono::milliseconds> (expiry_time - std::chrono::steady_clock::now ()).count();
	if(delay_ms < 0 ){
		delay_ms = 0;
	}
	rsnano::rsn_thread_pool_add_delayed_task (handle, delay_ms, execute_task, context, delete_task_context);
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (thread_pool & thread_pool, std::string const & name)
{
	auto composite = std::make_unique<container_info_composite> (name);
	// composite->add_component (std::make_unique<container_info_leaf> (container_info{ "count", thread_pool.num_queued_tasks (), sizeof (std::function<void ()>) }));
	return composite;
}

unsigned int nano::hardware_concurrency ()
{
	// Try to read overridden value from environment variable
	static int value = nano::get_env_int_or_default ("NANO_HARDWARE_CONCURRENCY", 0);
	if (value <= 0)
	{
		// Not present or invalid, use default
		return std::thread::hardware_concurrency ();
	}
	return value;
}

bool nano::join_or_pass (std::thread & thread)
{
	if (thread.joinable ())
	{
		thread.join ();
		return true;
	}
	else
	{
		return false;
	}
}
