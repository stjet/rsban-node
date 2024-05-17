#pragma once

#include "nano/lib/rsnano.hpp"

#include <nano/lib/config.hpp>
#include <nano/lib/numbers.hpp>

#include <functional>
#include <optional>
#include <vector>

namespace nano
{
class container_info_component;
class distributed_work;
class node;
class root;
class block;

class distributed_work_factory final
{
public:
	distributed_work_factory (nano::node &);
	distributed_work_factory (rsnano::DistributedWorkFactoryHandle * handle);
	distributed_work_factory (distributed_work_factory const &) = delete;
	distributed_work_factory (distributed_work_factory &&) = delete;
	~distributed_work_factory ();
	bool work_generation_enabled (bool secondary_work_peers = false) const;
	bool work_generation_enabled (std::vector<std::pair<std::string, uint16_t>> const & work_peers) const;
	std::optional<uint64_t> make_blocking (nano::block & block_a, uint64_t difficulty_a);
	std::optional<uint64_t> make_blocking (nano::work_version const version_a, nano::root const & root_a, uint64_t difficulty_a, std::optional<nano::account> const & account_a = std::nullopt);
	void make (nano::work_version const version_a, nano::root const & root_a, uint64_t difficulty_a, std::function<void (std::optional<uint64_t>)> callback_a, std::optional<nano::account> const & account_a = std::nullopt, bool const secondary_work_peers_a = false);
	void make (nano::work_version const version_a, nano::root const & root_a, std::vector<std::pair<std::string, uint16_t>> const & peers_a, uint64_t difficulty_a, std::function<void (std::optional<uint64_t>)> const & callback_a, std::optional<nano::account> const & account_a = std::nullopt);
	void cancel (nano::root const &);
	void stop ();

	rsnano::DistributedWorkFactoryHandle * handle;
};
}
