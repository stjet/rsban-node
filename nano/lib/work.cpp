#include <nano/crypto_lib/random_pool.hpp>
#include <nano/lib/blocks.hpp>
#include <nano/lib/epoch.hpp>
#include <nano/lib/thread_roles.hpp>
#include <nano/lib/threading.hpp>
#include <nano/lib/work.hpp>

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

nano::work_pool::work_pool (nano::network_constants & network_constants, unsigned max_threads_a, std::chrono::nanoseconds pow_rate_limiter_a)
{
	auto nw_constants_dto = network_constants.to_dto ();
	handle = rsnano::rsn_work_pool_create (&nw_constants_dto, max_threads_a, pow_rate_limiter_a.count ());
}

nano::work_pool::~work_pool ()
{
	rsnano::rsn_work_pool_destroy (handle);
}

void nano::work_pool::cancel (nano::root const & root_a)
{
	rsnano::rsn_work_pool_cancel (handle, root_a.bytes.data ());
}

void nano::work_pool::stop ()
{
	rsnano::rsn_work_pool_stop (handle);
}

void callback_work_done (void * context_a, uint64_t work_a, bool work_found_a)
{
	auto callback = static_cast<std::function<void (boost::optional<uint64_t> const &)> *> (context_a);
	boost::optional<uint64_t> work = work_found_a ? boost::optional<uint64_t> (work_a) : boost::none;
	(*callback) (work);
}

void delete_work_done_context (void * context_a)
{
	auto callback = static_cast<std::function<void (boost::optional<uint64_t> const &)> *> (context_a);
	delete callback;
}

void nano::work_pool::generate (nano::work_version const version_a, nano::root const & root_a, uint64_t difficulty_a, std::function<void (boost::optional<uint64_t> const &)> callback_a)
{
	auto context = new std::function<void (boost::optional<uint64_t> const &)> (callback_a);
	rsnano::rsn_work_pool_generate_async (handle, static_cast<uint8_t> (version_a), root_a.bytes.data (), difficulty_a, callback_work_done, context, delete_work_done_context);
}

boost::optional<uint64_t> nano::work_pool::generate (nano::root const & root_a)
{
	uint64_t result;
	auto has_result = rsnano::rsn_work_pool_generate_dev2 (handle, root_a.bytes.data (), &result);
	return has_result ? boost::optional<uint64_t> (result) : boost::none;
}

boost::optional<uint64_t> nano::work_pool::generate (nano::root const & root_a, uint64_t difficulty_a)
{
	uint64_t result;
	auto has_result = rsnano::rsn_work_pool_generate_dev (handle, root_a.bytes.data (), difficulty_a, &result);
	return has_result ? boost::optional<uint64_t> (result) : boost::none;
}

boost::optional<uint64_t> nano::work_pool::generate (nano::work_version const version_a, nano::root const & root_a, uint64_t difficulty_a)
{
	uint64_t result;
	auto has_result = rsnano::rsn_work_pool_generate (handle, static_cast<uint8_t> (version_a), root_a.bytes.data (), difficulty_a, &result);
	return has_result ? boost::optional<uint64_t> (result) : boost::none;
}

size_t nano::work_pool::size ()
{
	return rsnano::rsn_work_pool_size (handle);
}

size_t nano::work_pool::pending_size ()
{
	return rsnano::rsn_work_pool_size (handle);
}

size_t nano::work_pool::pending_value_size () const
{
	return rsnano::rsn_work_pool_pending_value_size ();
}

size_t nano::work_pool::thread_count () const
{
	return rsnano::rsn_work_pool_thread_count (handle);
}

bool nano::work_pool::has_opencl () const
{
	return rsnano::rsn_work_pool_has_opencl (handle);
}

bool nano::work_pool::work_generation_enabled () const
{
	return rsnano::rsn_work_pool_work_generation_enabled (handle);
}

uint64_t nano::work_pool::threshold_base (const nano::work_version version_a) const
{
	return rsnano::rsn_work_pool_threshold_base (handle, static_cast<uint8_t> (version_a));
}
uint64_t nano::work_pool::difficulty (const nano::work_version version_a, const nano::root & root_a, const uint64_t work_a) const
{
	return rsnano::rsn_work_pool_difficulty (handle, static_cast<uint8_t> (version_a), root_a.bytes.data (), work_a);
}
