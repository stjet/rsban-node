#include "nano/lib/rsnano.hpp"

#include <nano/node/distributed_work.hpp>
#include <nano/node/distributed_work_factory.hpp>
#include <nano/node/node.hpp>

nano::distributed_work_factory::distributed_work_factory (nano::node & node_a) :
	node (node_a),
	handle{ rsnano::rsn_distributed_work_factory_create (this) }
{
}

nano::distributed_work_factory::~distributed_work_factory ()
{
	stop ();
	rsnano::rsn_distributed_work_factory_destroy (handle);
}

bool nano::distributed_work_factory::work_generation_enabled () const
{
	return work_generation_enabled (node.config->work_peers);
}

bool nano::distributed_work_factory::work_generation_enabled (std::vector<std::pair<std::string, uint16_t>> const & work_peers) const
{
	return !work_peers.empty () || node.work.work_generation_enabled ();
}

std::optional<uint64_t> nano::distributed_work_factory::make_blocking (nano::block & block_a, uint64_t difficulty_a)
{
	auto opt_work_l (make_blocking (block_a.work_version (), block_a.root (), difficulty_a, block_a.account_field ()));
	if (opt_work_l.has_value ())
	{
		block_a.block_work_set (opt_work_l.value ());
	}
	return opt_work_l;
}

std::optional<uint64_t> nano::distributed_work_factory::make_blocking (nano::work_version const version_a, nano::root const & root_a, uint64_t difficulty_a, std::optional<nano::account> const & account_a)
{
	std::promise<std::optional<uint64_t>> promise;
	make (
	version_a, root_a, difficulty_a, [&promise] (std::optional<uint64_t> opt_work_a) {
		promise.set_value (opt_work_a);
	},
	account_a);
	return promise.get_future ().get ();
}

void nano::distributed_work_factory::make (nano::work_version const version_a, nano::root const & root_a, uint64_t difficulty_a, std::function<void (std::optional<uint64_t>)> callback_a, std::optional<nano::account> const & account_a, bool secondary_work_peers_a)
{
	auto const & peers_l (secondary_work_peers_a ? node.config->secondary_work_peers : node.config->work_peers);
	if (make (version_a, root_a, peers_l, difficulty_a, callback_a, account_a))
	{
		// Error in creating the job (either stopped or work generation is not possible)
		callback_a (std::nullopt);
	}
}

bool nano::distributed_work_factory::make (nano::work_version const version_a, nano::root const & root_a, std::vector<std::pair<std::string, uint16_t>> const & peers_a, uint64_t difficulty_a, std::function<void (std::optional<uint64_t>)> const & callback_a, std::optional<nano::account> const & account_a)
{
	return make (std::chrono::seconds (1), nano::work_request{ version_a, root_a, difficulty_a, account_a, callback_a, peers_a });
}

bool nano::distributed_work_factory::make (std::chrono::seconds const & backoff_a, nano::work_request const & request_a)
{
	bool error_l{ true };
	if (!stopped)
	{
		cleanup_finished ();
		if (work_generation_enabled (request_a.peers))
		{
			auto distributed (std::make_shared<nano::distributed_work> (node, request_a, backoff_a));
			{
				nano::lock_guard<nano::mutex> guard (mutex);
				items.emplace (request_a.root, distributed);
			}
			distributed->start ();
			error_l = false;
		}
	}
	return error_l;
}

void nano::distributed_work_factory::cancel (nano::root const & root_a)
{
	nano::lock_guard<nano::mutex> guard_l (mutex);
	auto root_items_l = items.equal_range (root_a);
	std::for_each (root_items_l.first, root_items_l.second, [] (auto item_l) {
		if (auto distributed_l = item_l.second.lock ())
		{
			// Send work_cancel to work peers and stop local work generation
			distributed_l->cancel ();
		}
	});
	items.erase (root_items_l.first, root_items_l.second);
}

void nano::distributed_work_factory::cleanup_finished ()
{
	nano::lock_guard<nano::mutex> guard (mutex);
	std::erase_if (items, [] (decltype (items)::value_type item) { return item.second.expired (); });
}

void nano::distributed_work_factory::stop ()
{
	if (!stopped.exchange (true))
	{
		// Cancel any ongoing work
		nano::lock_guard<nano::mutex> guard (mutex);
		for (auto & item_l : items)
		{
			if (auto distributed_l = item_l.second.lock ())
			{
				distributed_l->cancel ();
			}
		}
		items.clear ();
	}
}

std::size_t nano::distributed_work_factory::size () const
{
	nano::lock_guard<nano::mutex> guard_l (mutex);
	return items.size ();
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (distributed_work_factory & distributed_work, std::string const & name)
{
	auto item_count = distributed_work.size ();
	auto sizeof_item_element = sizeof (decltype (nano::distributed_work_factory::items)::value_type);
	auto composite = std::make_unique<container_info_composite> (name);
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "items", item_count, sizeof_item_element }));
	return composite;
}
