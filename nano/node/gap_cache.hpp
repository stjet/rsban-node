#pragma once

#include "nano/lib/rsnano.hpp"

#include <nano/lib/numbers.hpp>
#include <nano/lib/utility.hpp>
#include <nano/secure/common.hpp>

#include <boost/multi_index/hashed_index.hpp>
#include <boost/multi_index/member.hpp>
#include <boost/multi_index/ordered_index.hpp>
#include <boost/multi_index/sequenced_index.hpp>
#include <boost/multi_index_container.hpp>

#include <chrono>
#include <memory>
#include <mutex>
#include <vector>

namespace nano
{
class node;
class ledger;

/** For each gap in account chains, track arrival time and voters */
class gap_information final
{
public:
	std::chrono::system_clock::time_point arrival;
	nano::block_hash hash;
	std::vector<nano::account> voters;
	bool bootstrap_started{ false };
};

/** Maintains voting and arrival information for gaps (missing source or previous blocks in account chains) */
class gap_cache final
{
public:
	gap_cache (nano::node &);
	gap_cache (gap_cache const &) = delete;
	gap_cache (gap_cache &&) = delete;
	~gap_cache ();
	void add (nano::block_hash const &, std::chrono::system_clock::time_point = std::chrono::system_clock::now ());
	void erase (nano::block_hash const & hash_a);
	void vote (std::shared_ptr<nano::vote> const &);
	bool bootstrap_check (std::vector<nano::account> const &, nano::block_hash const &);
	void bootstrap_start (nano::block_hash const & hash_a);
	nano::uint128_t bootstrap_threshold ();
	std::size_t size ();
	bool block_exists (nano::block_hash const & hash_a);
	std::chrono::system_clock::time_point earliest ();
	std::chrono::system_clock::time_point block_arrival (nano::block_hash const & hash_a);
	// clang-format on
	std::size_t const max = 256;
	nano::node & node;
	std::function<void (nano::block_hash const &)> start_bootstrap_callback;
	rsnano::GapCacheHandle * handle;
};

std::unique_ptr<container_info_component> collect_container_info (gap_cache & gap_cache, std::string const & name);
}
