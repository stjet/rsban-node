#pragma once

#include <nano/lib/config.hpp>
#include <nano/lib/numbers.hpp>

#include <atomic>
#include <functional>
#include <optional>
#include <unordered_map>
#include <vector>

namespace nano
{
class container_info_component;
class distributed_work;
class node;
class root;
class block;
struct work_request;

class distributed_work_factory final
{
public:
	distributed_work_factory (nano::node &);
	distributed_work_factory (distributed_work_factory const &) = delete;
	distributed_work_factory (distributed_work_factory &&) = delete;
	~distributed_work_factory ();
	bool work_generation_enabled () const;
	bool work_generation_enabled (std::vector<std::pair<std::string, uint16_t>> const & work_peers) const;
	std::optional<uint64_t> make_blocking (nano::block & block_a, uint64_t difficulty_a);
	std::optional<uint64_t> make_blocking (nano::work_version const version_a, nano::root const & root_a, uint64_t difficulty_a, std::optional<nano::account> const & account_a = std::nullopt);
	void make (nano::work_version const version_a, nano::root const & root_a, uint64_t difficulty_a, std::function<void (std::optional<uint64_t>)> callback_a, std::optional<nano::account> const & account_a = std::nullopt, bool const secondary_work_peers_a = false);
	bool make (nano::work_version const version_a, nano::root const & root_a, std::vector<std::pair<std::string, uint16_t>> const & peers_a, uint64_t difficulty_a, std::function<void (std::optional<uint64_t>)> const & callback_a, std::optional<nano::account> const & account_a = std::nullopt);
	bool make (std::chrono::seconds const &, nano::work_request const &);
	void cancel (nano::root const &);
	void cleanup_finished ();
	void stop ();
	std::size_t size () const;

private:
	std::unordered_multimap<nano::root, std::weak_ptr<nano::distributed_work>> items;

	nano::node & node;
	mutable nano::mutex mutex;
	std::atomic<bool> stopped{ false };

	friend std::unique_ptr<container_info_component> collect_container_info (distributed_work_factory &, std::string const &);

public:
	rsnano::DistributedWorkFactoryHandle * handle;
};

std::unique_ptr<container_info_component> collect_container_info (distributed_work_factory & distributed_work, std::string const & name);
}
