<<<<<<< HEAD
#include "nano/lib/rsnano.hpp"
#include "nano/lib/thread_roles.hpp"

#include <nano/boost/asio/post.hpp>
=======
>>>>>>> 4fdc0ce08e4cd30ee54d2b473d3fbd7ac2cb0cfd
#include <nano/lib/config.hpp>
#include <nano/lib/threading.hpp>

#include <boost/asio/post.hpp>
#include <boost/asio/steady_timer.hpp>
#include <boost/asio/thread_pool.hpp>
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
	int64_t delay_ms = std::chrono::duration_cast<std::chrono::milliseconds> (expiry_time - std::chrono::steady_clock::now ()).count ();
	if (delay_ms < 0)
	{
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
