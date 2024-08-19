#pragma once

#include <nano/lib/locks.hpp>
#include <nano/lib/numbers.hpp>
#include <nano/lib/observer_set.hpp>
#include <nano/secure/common.hpp>

#include <boost/multi_index/member.hpp>
#include <boost/multi_index/ordered_index.hpp>
#include <boost/multi_index/random_access_index.hpp>
#include <boost/multi_index/sequenced_index.hpp>
#include <boost/multi_index_container.hpp>

namespace nano
{
class stats;

class unchecked_map
{
public:
	unchecked_map (unsigned const max_unchecked_blocks, nano::stats &, bool do_delete);
	explicit unchecked_map (rsnano::UncheckedMapHandle * handle);
	~unchecked_map ();

	void put (nano::hash_or_account const & dependency, nano::unchecked_info const & info);
	void for_each (
	std::function<void (nano::unchecked_key const &, nano::unchecked_info const &)> action, std::function<bool ()> predicate = [] () { return true; });
	void for_each (
	nano::hash_or_account const & dependency, std::function<void (nano::unchecked_key const &, nano::unchecked_info const &)> action, std::function<bool ()> predicate = [] () { return true; });
	std::vector<nano::unchecked_info> get (nano::block_hash const &);
	void del (nano::unchecked_key const & key);
	void clear ();
	std::size_t count () const;

	rsnano::UncheckedMapHandle * handle;
};
}
