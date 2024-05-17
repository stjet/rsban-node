#include <nano/lib/rsnano.hpp>
#include <nano/node/distributed_work_factory.hpp>
#include <nano/node/node.hpp>

nano::distributed_work_factory::distributed_work_factory (nano::node & node_a) :
	handle{ rsnano::rsn_distributed_work_factory_create (node_a.work.handle, node_a.async_rt.handle) }
{
}

nano::distributed_work_factory::~distributed_work_factory ()
{
	rsnano::rsn_distributed_work_factory_destroy (handle);
}

bool nano::distributed_work_factory::work_generation_enabled (bool secondary_work_peers) const
{
	return rsnano::rsn_distributed_work_factory_enabled (handle);
}

bool nano::distributed_work_factory::work_generation_enabled (std::vector<std::pair<std::string, uint16_t>> const & work_peers) const
{
	return rsnano::rsn_distributed_work_factory_enabled (handle);
}

std::optional<uint64_t> nano::distributed_work_factory::make_blocking (nano::block & block_a, uint64_t difficulty_a)
{
	uint64_t result;
	if (rsnano::rsn_distributed_work_factory_make_blocking_block (handle, block_a.get_handle (), difficulty_a, &result))
	{
		return { result };
	}
	else
	{
		return std::nullopt;
	}
}

std::optional<uint64_t> nano::distributed_work_factory::make_blocking (nano::work_version const version_a, nano::root const & root_a, uint64_t difficulty_a, std::optional<nano::account> const & account_a)
{
	uint64_t result;
	const uint8_t * account_ptr = nullptr;
	if (account_a.has_value ())
	{
		account_ptr = account_a.value ().bytes.data ();
	}
	if (rsnano::rsn_distributed_work_factory_make_blocking (handle, root_a.bytes.data (), difficulty_a, account_ptr, &result))
	{
		return { result };
	}
	else
	{
		return std::nullopt;
	}
}

namespace
{
void callback_wrapper (void * context, bool has_result, uint64_t work)
{
	auto callback = static_cast<std::function<void (std::optional<uint64_t>)> *> (context);
	std::optional<uint64_t> opt_work{};
	if (has_result)
	{
		opt_work = { work };
	}
	(*callback) (opt_work);
}

void delete_context (void * context)
{
	auto callback = static_cast<std::function<void (std::optional<uint64_t>)> *> (context);
	delete callback;
}
}

void nano::distributed_work_factory::make (nano::work_version const version_a, nano::root const & root_a, uint64_t difficulty_a, std::function<void (std::optional<uint64_t>)> callback_a, std::optional<nano::account> const & account_a, bool secondary_work_peers_a)
{
	const uint8_t * account_ptr = nullptr;
	if (account_a.has_value ())
	{
		account_ptr = account_a.value ().bytes.data ();
	}
	auto context = new std::function<void (std::optional<uint64_t>)> (std::move (callback_a));
	rsnano::rsn_distributed_work_factory_make (handle, root_a.bytes.data (), difficulty_a, account_ptr, callback_wrapper, context, delete_context);
}

void nano::distributed_work_factory::make (nano::work_version const version_a, nano::root const & root_a, std::vector<std::pair<std::string, uint16_t>> const & peers_a, uint64_t difficulty_a, std::function<void (std::optional<uint64_t>)> const & callback_a, std::optional<nano::account> const & account_a)
{
	make (version_a, root_a, difficulty_a, callback_a, false);
}

void nano::distributed_work_factory::cancel (nano::root const & root_a)
{
	rsnano::rsn_distributed_work_factory_cancel (handle, root_a.bytes.data ());
}

void nano::distributed_work_factory::stop ()
{
}
