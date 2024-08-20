#include "nano/lib/rsnano.hpp"
#include "nano/secure/common.hpp"

#include <nano/lib/blocks.hpp>
#include <nano/lib/locks.hpp>
#include <nano/lib/stats.hpp>
#include <nano/lib/stats_enums.hpp>
#include <nano/lib/thread_roles.hpp>
#include <nano/lib/timer.hpp>
#include <nano/node/unchecked_map.hpp>

namespace
{
void action_callback_wrapper (void * context, rsnano::UncheckedKeyDto * key, rsnano::UncheckedInfoHandle * info)
{
	auto fn = static_cast<std::function<void (nano::unchecked_key const &, nano::unchecked_info const &)> *> (context);
	nano::unchecked_info i{ rsnano::rsn_unchecked_info_clone (info) };
	nano::unchecked_key k{ *key };
	(*fn) (k, i);
}

void drop_action_callback (void * context_a)
{
	auto fn = static_cast<std::function<void (nano::unchecked_key const &, nano::unchecked_info const &)> *> (context_a);
	delete fn;
}

bool predicate_callback_wrapper (void * context_a)
{
	auto fn = static_cast<std::function<bool ()> *> (context_a);
	return (*fn) ();
}

void drop_predicate_callback (void * context_a)
{
	auto fn = static_cast<std::function<bool ()> *> (context_a);
	delete fn;
}
}

nano::unchecked_map::unchecked_map (unsigned const max_unchecked_blocks, nano::stats & stats, bool disable_delete)
{
	handle = rsnano::rsn_unchecked_map_create (max_unchecked_blocks, stats.handle, disable_delete);
}

nano::unchecked_map::unchecked_map (rsnano::UncheckedMapHandle * handle) :
	handle{ handle }
{
}

nano::unchecked_map::~unchecked_map ()
{
	rsnano::rsn_unchecked_map_destroy (handle);
}

void nano::unchecked_map::put (nano::hash_or_account const & dependency, nano::unchecked_info const & info)
{
	rsnano::rsn_unchecked_map_put (handle, dependency.bytes.data (), info.handle);
}

void nano::unchecked_map::for_each (std::function<void (nano::unchecked_key const &, nano::unchecked_info const &)> action, std::function<bool ()> predicate)
{
	rsnano::rsn_unchecked_map_for_each1 (handle,
	action_callback_wrapper,
	new std::function<void (nano::unchecked_key const &, nano::unchecked_info const &)>{ action },
	drop_action_callback,
	predicate_callback_wrapper,
	new std::function<bool ()>{ predicate },
	drop_predicate_callback);
}

void nano::unchecked_map::for_each (nano::hash_or_account const & dependency, std::function<void (nano::unchecked_key const &, nano::unchecked_info const &)> action, std::function<bool ()> predicate)
{
	rsnano::rsn_unchecked_map_for_each2 (handle, dependency.bytes.data (),
	action_callback_wrapper,
	new std::function<void (nano::unchecked_key const &, nano::unchecked_info const &)>{ action },
	drop_action_callback,
	predicate_callback_wrapper,
	new std::function<bool ()>{ predicate },
	drop_predicate_callback);
}

std::vector<nano::unchecked_info> nano::unchecked_map::get (nano::block_hash const & hash)
{
	std::vector<nano::unchecked_info> result;
	for_each (hash, [&result] (nano::unchecked_key const & key, nano::unchecked_info const & info) {
		result.push_back (info);
	});
	return result;
}

void nano::unchecked_map::del (nano::unchecked_key const & key)
{
	rsnano::rsn_unchecked_map_del (handle, key.to_dto ());
}

void nano::unchecked_map::clear ()
{
	rsnano::rsn_unchecked_map_clear (handle);
}

std::size_t nano::unchecked_map::count () const
{
	return rsnano::rsn_unchecked_map_entries_count (handle);
}
