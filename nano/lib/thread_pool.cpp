#include "nano/lib/rsnano.hpp"

#include <nano/lib/thread_pool.hpp>

#include <iostream>

/*
 * thread_pool
 */

nano::thread_pool::thread_pool (unsigned num_threads, nano::thread_role::name thread_name) :
	handle{ rsnano::rsn_thread_pool_create (num_threads, nano::thread_role::get_string (thread_name).c_str ()) }
{
}

nano::thread_pool::thread_pool (rsnano::ThreadPoolHandle * handle) :
	handle{ handle }
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
