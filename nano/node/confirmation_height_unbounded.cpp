#include <nano/lib/stats.hpp>
#include <nano/node/confirmation_height_unbounded.hpp>
#include <nano/node/logging.hpp>
#include <nano/node/write_database_queue.hpp>
#include <nano/secure/ledger.hpp>

#include <boost/format.hpp>

#include <numeric>

nano::confirmation_height_unbounded::confirmation_height_unbounded (nano::ledger & ledger_a, nano::stat & stats_a, nano::write_database_queue & write_database_queue_a, std::chrono::milliseconds batch_separate_pending_min_time_a, nano::logging const & logging_a, nano::logger_mt & logger_a, uint64_t & batch_write_size_a, std::function<void (std::vector<std::shared_ptr<nano::block>> const &)> const & notify_observers_callback_a, std::function<void (nano::block_hash const &)> const & notify_block_already_cemented_observers_callback_a, std::function<uint64_t ()> const & awaiting_processing_size_callback_a) :
	handle{ rsnano::rsn_conf_height_unbounded_create () },
	ledger (ledger_a),
	stats (stats_a),
	write_database_queue (write_database_queue_a),
	batch_separate_pending_min_time (batch_separate_pending_min_time_a),
	logging (logging_a),
	logger (logger_a),
	batch_write_size (batch_write_size_a),
	notify_observers_callback (notify_observers_callback_a),
	notify_block_already_cemented_observers_callback (notify_block_already_cemented_observers_callback_a),
	awaiting_processing_size_callback (awaiting_processing_size_callback_a)
{
}

nano::confirmation_height_unbounded::~confirmation_height_unbounded ()
{
	rsnano::rsn_conf_height_unbounded_destroy (handle);
}

void nano::confirmation_height_unbounded::process (std::shared_ptr<nano::block> original_block)
{
	if (pending_empty ())
	{
		clear_process_vars ();
		timer.restart ();
	}
	conf_height_details_shared_ptr receive_details;
	auto current = original_block->hash ();
	std::vector<nano::block_hash> orig_block_callback_data;

	std::vector<receive_source_pair> receive_source_pairs;
	release_assert (receive_source_pairs.empty ());

	bool first_iter = true;
	auto read_transaction (ledger.store.tx_begin_read ());

	do
	{
		if (!receive_source_pairs.empty ())
		{
			receive_details = receive_source_pairs.back ().receive_details;
			current = receive_source_pairs.back ().source_hash;
		}
		else
		{
			// If receive_details is set then this is the final iteration and we are back to the original chain.
			// We need to confirm any blocks below the original hash (incl self) and the first receive block
			// (if the original block is not already a receive)
			if (!receive_details.is_null ())
			{
				current = original_block->hash ();
				receive_details.destroy ();
			}
		}

		std::shared_ptr<nano::block> block;
		if (first_iter)
		{
			debug_assert (current == original_block->hash ());
			// This is the original block passed so can use it directly
			block = original_block;
			nano::lock_guard<nano::mutex> guard (block_cache_mutex);
			block_cache[original_block->hash ()] = original_block;
		}
		else
		{
			block = get_block_and_sideband (current, *read_transaction);
		}
		if (!block)
		{
			auto error_str = (boost::format ("Ledger mismatch trying to set confirmation height for block %1% (unbounded processor)") % current.to_string ()).str ();
			logger.always_log (error_str);
			std::cerr << error_str << std::endl;
		}
		release_assert (block);

		nano::account account (block->account ());
		if (account.is_zero ())
		{
			account = block->sideband ().account ();
		}

		auto block_height = block->sideband ().height ();
		uint64_t confirmation_height = 0;
		rsnano::ConfirmedIteratedPairsIteratorDto account_it;
		rsnano::rsn_conf_height_unbounded_conf_iterated_pairs_find (handle, account.bytes.data (), &account_it);
		if (!account_it.is_end)
		{
			confirmation_height = account_it.confirmed_height;
		}
		else
		{
			nano::confirmation_height_info confirmation_height_info;
			ledger.store.confirmation_height ().get (*read_transaction, account, confirmation_height_info);
			confirmation_height = confirmation_height_info.height ();

			// This block was added to the confirmation height processor but is already confirmed
			if (first_iter && confirmation_height >= block_height)
			{
				debug_assert (current == original_block->hash ());
				notify_block_already_cemented_observers_callback (original_block->hash ());
			}
		}
		auto iterated_height = confirmation_height;
		if (!account_it.is_end && account_it.iterated_height > iterated_height)
		{
			iterated_height = account_it.iterated_height;
		}

		auto count_before_receive = receive_source_pairs.size ();
		std::vector<nano::block_hash> block_callback_datas_required;
		auto already_traversed = iterated_height >= block_height;
		if (!already_traversed)
		{
			collect_unconfirmed_receive_and_sources_for_account (block_height, iterated_height, block, current, account, *read_transaction, receive_source_pairs, block_callback_datas_required, orig_block_callback_data, original_block);
		}

		// Exit early when the processor has been stopped, otherwise this function may take a
		// while (and hence keep the process running) if updating a long chain.
		if (stopped)
		{
			break;
		}

		// No longer need the read transaction
		read_transaction->reset ();

		// If this adds no more open or receive blocks, then we can now confirm this account as well as the linked open/receive block
		// Collect as pending any writes to the database and do them in bulk after a certain time.
		auto confirmed_receives_pending = (count_before_receive != receive_source_pairs.size ());
		if (!confirmed_receives_pending)
		{
			preparation_data preparation_data{ block_height, confirmation_height, iterated_height, account_it, account, receive_details, already_traversed, current, block_callback_datas_required, orig_block_callback_data };
			prepare_iterated_blocks_for_cementing (preparation_data);

			if (!receive_source_pairs.empty ())
			{
				// Pop from the end
				receive_source_pairs.erase (receive_source_pairs.end () - 1);
			}
		}
		else if (block_height > iterated_height)
		{
			if (!account_it.is_end)
			{
				rsnano::rsn_conf_height_unbounded_conf_iterated_pairs_set_iterated_height (handle, &account_it.account[0], block_height);
			}
			else
			{
				rsnano::rsn_conf_height_unbounded_conf_iterated_pairs_insert (handle, account.bytes.data (), confirmation_height, block_height);
			}
		}

		auto max_write_size_reached = (rsnano::rsn_conf_height_unbounded_pending_writes_size (handle) >= confirmation_height::unbounded_cutoff);
		// When there are a lot of pending confirmation height blocks, it is more efficient to
		// bulk some of them up to enable better write performance which becomes the bottleneck.
		auto min_time_exceeded = (timer.since_start () >= batch_separate_pending_min_time);
		auto finished_iterating = receive_source_pairs.empty ();
		auto no_pending = awaiting_processing_size_callback () == 0;
		auto should_output = finished_iterating && (no_pending || min_time_exceeded);

		auto total_pending_write_block_count = rsnano::rsn_conf_height_unbounded_total_pending_write_block_count (handle);
		auto force_write = total_pending_write_block_count > batch_write_size;

		if ((max_write_size_reached || should_output || force_write) && rsnano::rsn_conf_height_unbounded_pending_writes_size (handle) > 0)
		{
			if (write_database_queue.process (nano::writer::confirmation_height))
			{
				auto scoped_write_guard = write_database_queue.pop ();
				cement_blocks (scoped_write_guard);
			}
			else if (force_write)
			{
				// Unbounded processor has grown too large, force a write
				auto scoped_write_guard = write_database_queue.wait (nano::writer::confirmation_height);
				cement_blocks (scoped_write_guard);
			}
		}

		first_iter = false;
		read_transaction->renew ();
	} while ((!receive_source_pairs.empty () || current != original_block->hash ()) && !stopped);
}

void nano::confirmation_height_unbounded::collect_unconfirmed_receive_and_sources_for_account (uint64_t block_height_a, uint64_t confirmation_height_a, std::shared_ptr<nano::block> const & block_a, nano::block_hash const & hash_a, nano::account const & account_a, nano::read_transaction const & transaction_a, std::vector<receive_source_pair> & receive_source_pairs_a, std::vector<nano::block_hash> & block_callback_data_a, std::vector<nano::block_hash> & orig_block_callback_data_a, std::shared_ptr<nano::block> original_block)
{
	debug_assert (block_a->hash () == hash_a);
	auto hash (hash_a);
	auto num_to_confirm = block_height_a - confirmation_height_a;

	// Handle any sends above a receive
	auto is_original_block = (hash == original_block->hash ());
	auto hit_receive = false;
	auto first_iter = true;
	while ((num_to_confirm > 0) && !hash.is_zero () && !stopped)
	{
		std::shared_ptr<nano::block> block;
		if (first_iter)
		{
			debug_assert (hash == hash_a);
			block = block_a;
			nano::lock_guard<nano::mutex> guard (block_cache_mutex);
			block_cache[hash] = block_a;
		}
		else
		{
			block = get_block_and_sideband (hash, transaction_a);
		}

		if (block)
		{
			auto source (block->source ());
			if (source.is_zero ())
			{
				source = block->link ().as_block_hash ();
			}

			if (!source.is_zero () && !ledger.is_epoch_link (source) && ledger.store.block ().exists (transaction_a, source))
			{
				if (!hit_receive && !block_callback_data_a.empty ())
				{
					// Add the callbacks to the associated receive to retrieve later
					debug_assert (!receive_source_pairs_a.empty ());
					auto & last_receive_details = receive_source_pairs_a.back ().receive_details;
					last_receive_details.set_source_block_callback_data (block_callback_data_a);
					block_callback_data_a.clear ();
				}

				is_original_block = false;
				hit_receive = true;

				auto block_height = confirmation_height_a + num_to_confirm;
				conf_height_details details (account_a, hash, block_height, 1, std::vector<nano::block_hash>{ hash });
				auto shared_details = rsnano::rsn_conf_height_details_shared_ptr_create (details.handle);
				receive_source_pairs_a.emplace_back (shared_details, source);
			}
			else if (is_original_block)
			{
				orig_block_callback_data_a.push_back (hash);
			}
			else
			{
				if (!hit_receive)
				{
					// This block is cemented via a recieve, as opposed to below a receive being cemented
					block_callback_data_a.push_back (hash);
				}
				else
				{
					// We have hit a receive before, add the block to it
					auto & last_receive_details = receive_source_pairs_a.back ().receive_details;
					last_receive_details.set_num_blocks_confirmed (last_receive_details.get_num_blocks_confirmed () + 1);
					last_receive_details.add_block_callback_data (hash);

					implicit_receive_cemented_mapping[hash] = conf_height_details_weak_ptr (last_receive_details);
					implicit_receive_cemented_mapping_size = implicit_receive_cemented_mapping.size ();
				}
			}

			hash = block->previous ();
		}

		--num_to_confirm;
		first_iter = false;
	}
}

void nano::confirmation_height_unbounded::prepare_iterated_blocks_for_cementing (preparation_data & preparation_data_a)
{
	auto receive_details = preparation_data_a.receive_details;
	auto block_height = preparation_data_a.block_height;
	if (block_height > preparation_data_a.confirmation_height)
	{
		// Check whether the previous block has been seen. If so, the rest of sends below have already been seen so don't count them
		if (!preparation_data_a.account_it.is_end)
		{
			rsnano::rsn_conf_height_unbounded_conf_iterated_pairs_set_confirmed_height (handle, &preparation_data_a.account_it.account[0], block_height);
			if (block_height > preparation_data_a.iterated_height)
			{
				rsnano::rsn_conf_height_unbounded_conf_iterated_pairs_set_iterated_height (handle, &preparation_data_a.account_it.account[0], block_height);
			}
		}
		else
		{
			rsnano::rsn_conf_height_unbounded_conf_iterated_pairs_insert (handle, preparation_data_a.account.bytes.data (), block_height, block_height);
		}

		auto num_blocks_confirmed = block_height - preparation_data_a.confirmation_height;
		auto block_callback_data = preparation_data_a.block_callback_data;
		if (block_callback_data.empty ())
		{
			if (receive_details.is_null ())
			{
				block_callback_data = preparation_data_a.orig_block_callback_data;
			}
			else
			{
				if (preparation_data_a.already_traversed && receive_details.get_source_block_callback_data ().empty ())
				{
					// We are confirming a block which has already been traversed and found no associated receive details for it.
					auto & above_receive_details_w = implicit_receive_cemented_mapping[preparation_data_a.current];
					debug_assert (!above_receive_details_w.expired ());
					auto above_receive_details = above_receive_details_w.upgrade ();

					auto num_blocks_already_confirmed = above_receive_details.get_num_blocks_confirmed () - (above_receive_details.get_height () - preparation_data_a.confirmation_height);

					auto block_data{ above_receive_details.get_block_callback_data () };
					auto end_it = block_data.begin () + block_data.size () - (num_blocks_already_confirmed);
					auto start_it = end_it - num_blocks_confirmed;

					block_callback_data.assign (start_it, end_it);
				}
				else
				{
					block_callback_data = receive_details.get_source_block_callback_data ();
				}

				auto num_to_remove = block_callback_data.size () - num_blocks_confirmed;
				block_callback_data.erase (std::next (block_callback_data.rbegin (), num_to_remove).base (), block_callback_data.end ());
				receive_details.set_source_block_callback_data (std::vector<nano::block_hash>{});
			}
		}

		nano::confirmation_height_unbounded::conf_height_details details{ preparation_data_a.account, preparation_data_a.current, block_height, num_blocks_confirmed, block_callback_data };
		rsnano::rsn_conf_height_unbounded_pending_writes_add (handle, details.handle);
	}

	if (!receive_details.is_null ())
	{
		// Check whether the previous block has been seen. If so, the rest of sends below have already been seen so don't count them
		auto receive_account = receive_details.get_account ();
		rsnano::ConfirmedIteratedPairsIteratorDto receive_account_it;
		rsnano::rsn_conf_height_unbounded_conf_iterated_pairs_find (handle, receive_account.bytes.data (), &receive_account_it);
		if (!receive_account_it.is_end)
		{
			// Get current height
			auto current_height = receive_account_it.confirmed_height;
			rsnano::rsn_conf_height_unbounded_conf_iterated_pairs_set_confirmed_height (handle, receive_account.bytes.data (), receive_details.get_height ());
			auto const orig_num_blocks_confirmed = receive_details.get_num_blocks_confirmed ();
			receive_details.set_num_blocks_confirmed (receive_details.get_height () - current_height);

			// Get the difference and remove the callbacks
			auto block_callbacks_to_remove = orig_num_blocks_confirmed - receive_details.get_num_blocks_confirmed ();
			auto tmp_blocks{ receive_details.get_block_callback_data () };
			tmp_blocks.erase (std::next (tmp_blocks.rbegin (), block_callbacks_to_remove).base (), tmp_blocks.end ());
			receive_details.set_block_callback_data (tmp_blocks);
			debug_assert (receive_details.get_block_callback_data ().size () == receive_details.get_num_blocks_confirmed ());
		}
		else
		{
			rsnano::rsn_conf_height_unbounded_conf_iterated_pairs_insert (handle, receive_account.bytes.data (), receive_details.get_height (), receive_details.get_height ());
		}

		rsnano::rsn_conf_height_unbounded_pending_writes_add2 (handle, receive_details.handle);
	}
}

void nano::confirmation_height_unbounded::cement_blocks (nano::write_guard & scoped_write_guard_a)
{
	nano::timer<std::chrono::milliseconds> cemented_batch_timer;
	std::vector<std::shared_ptr<nano::block>> cemented_blocks;
	auto error = false;
	{
		auto transaction (ledger.store.tx_begin_write ({}, { nano::tables::confirmation_height }));
		cemented_batch_timer.start ();
		while (rsnano::rsn_conf_height_unbounded_pending_writes_size (handle) > 0)
		{
			nano::confirmation_height_unbounded::conf_height_details pending{ rsnano::rsn_conf_height_unbounded_pending_writes_front (handle) };
			nano::confirmation_height_info confirmation_height_info;
			ledger.store.confirmation_height ().get (*transaction, pending.get_account (), confirmation_height_info);
			auto confirmation_height = confirmation_height_info.height ();
			if (pending.get_height () > confirmation_height)
			{
				auto block = ledger.store.block ().get (*transaction, pending.get_hash ());
				debug_assert (ledger.pruning_enabled () || block != nullptr);
				debug_assert (ledger.pruning_enabled () || block->sideband ().height () == pending.get_height ());

				if (!block)
				{
					if (ledger.pruning_enabled () && ledger.store.pruned ().exists (*transaction, pending.get_hash ()))
					{
						rsnano::rsn_conf_height_unbounded_pending_writes_erase_first (handle);
						continue;
					}
					else
					{
						auto error_str = (boost::format ("Failed to write confirmation height for block %1% (unbounded processor)") % pending.get_hash ().to_string ()).str ();
						logger.always_log (error_str);
						std::cerr << error_str << std::endl;
						error = true;
						break;
					}
				}
				stats.add (nano::stat::type::confirmation_height, nano::stat::detail::blocks_confirmed, nano::stat::dir::in, pending.get_height () - confirmation_height);
				stats.add (nano::stat::type::confirmation_height, nano::stat::detail::blocks_confirmed_unbounded, nano::stat::dir::in, pending.get_height () - confirmation_height);
				debug_assert (pending.get_num_blocks_confirmed () == pending.get_height () - confirmation_height);
				confirmation_height = pending.get_height ();
				ledger.cache.add_cemented (pending.get_num_blocks_confirmed ());
				ledger.store.confirmation_height ().put (*transaction, pending.get_account (), { confirmation_height, pending.get_hash () });

				// Reverse it so that the callbacks start from the lowest newly cemented block and move upwards
				auto tmp_blocks{ pending.get_block_callback_data () };
				std::reverse (tmp_blocks.begin (), tmp_blocks.end ());
				pending.set_block_callback_data (tmp_blocks);

				nano::lock_guard<nano::mutex> guard (block_cache_mutex);
				tmp_blocks = pending.get_block_callback_data ();
				std::transform (tmp_blocks.begin (), tmp_blocks.end (), std::back_inserter (cemented_blocks), [&block_cache = block_cache] (auto const & hash_a) {
					debug_assert (block_cache.count (hash_a) == 1);
					return block_cache.at (hash_a);
				});
			}
			rsnano::rsn_conf_height_unbounded_pending_writes_erase_first (handle);
		}
	}

	auto time_spent_cementing = cemented_batch_timer.since_start ().count ();
	if (logging.timing_logging () && time_spent_cementing > 50)
	{
		logger.always_log (boost::str (boost::format ("Cemented %1% blocks in %2% %3% (unbounded processor)") % cemented_blocks.size () % time_spent_cementing % cemented_batch_timer.unit ()));
	}

	scoped_write_guard_a.release ();
	notify_observers_callback (cemented_blocks);
	release_assert (!error);

	debug_assert (rsnano::rsn_conf_height_unbounded_pending_writes_size (handle) == 0);
	debug_assert (rsnano::rsn_conf_height_unbounded_pending_writes_len (handle) == 0);
	timer.restart ();
}

std::shared_ptr<nano::block> nano::confirmation_height_unbounded::get_block_and_sideband (nano::block_hash const & hash_a, nano::transaction const & transaction_a)
{
	nano::lock_guard<nano::mutex> guard (block_cache_mutex);
	auto block_cache_it = block_cache.find (hash_a);
	if (block_cache_it != block_cache.cend ())
	{
		return block_cache_it->second;
	}
	else
	{
		auto block (ledger.store.block ().get (transaction_a, hash_a));
		block_cache.emplace (hash_a, block);
		return block;
	}
}

bool nano::confirmation_height_unbounded::pending_empty () const
{
	return rsnano::rsn_conf_height_unbounded_pending_empty (handle);
}

void nano::confirmation_height_unbounded::clear_process_vars ()
{
	// Separate blocks which are pending confirmation height can be batched by a minimum processing time (to improve lmdb disk write performance),
	// so make sure the slate is clean when a new batch is starting.
	rsnano::rsn_conf_height_unbounded_conf_iterated_pairs_clear (handle);
	implicit_receive_cemented_mapping.clear ();
	implicit_receive_cemented_mapping_size = 0;
	{
		nano::lock_guard<nano::mutex> guard (block_cache_mutex);
		block_cache.clear ();
	}
}

bool nano::confirmation_height_unbounded::has_iterated_over_block (nano::block_hash const & hash_a) const
{
	nano::lock_guard<nano::mutex> guard (block_cache_mutex);
	return block_cache.count (hash_a) == 1;
}

void nano::confirmation_height_unbounded::stop ()
{
	stopped = true;
}

uint64_t nano::confirmation_height_unbounded::block_cache_size () const
{
	nano::lock_guard<nano::mutex> guard (block_cache_mutex);
	return block_cache.size ();
}

nano::confirmation_height_unbounded::conf_height_details::conf_height_details (nano::account const & account_a, nano::block_hash const & hash_a, uint64_t height_a, uint64_t num_blocks_confirmed_a, std::vector<nano::block_hash> const & block_callback_data_a) :
	handle{ rsnano::rsn_conf_height_details_create (account_a.bytes.data (), hash_a.bytes.data (), height_a, num_blocks_confirmed_a) }
{
	for (auto b : block_callback_data_a)
	{
		add_block_callback_data (b);
	}
}

nano::confirmation_height_unbounded::conf_height_details::conf_height_details (nano::confirmation_height_unbounded::conf_height_details const & other_a) :
	handle{ rsnano::rsn_conf_height_details_clone (other_a.handle) }
{
}

nano::confirmation_height_unbounded::conf_height_details::~conf_height_details ()
{
	rsnano::rsn_conf_height_details_destroy (handle);
}

nano::confirmation_height_unbounded::conf_height_details & nano::confirmation_height_unbounded::conf_height_details::operator= (nano::confirmation_height_unbounded::conf_height_details const & other_a)
{
	rsnano::rsn_conf_height_details_destroy (handle);
	handle = rsnano::rsn_conf_height_details_clone (other_a.handle);
	return *this;
}

nano::account nano::confirmation_height_unbounded::conf_height_details::get_account () const
{
	nano::account account;
	rsnano::rsn_conf_height_details_account (handle, account.bytes.data ());
	return account;
}
nano::block_hash nano::confirmation_height_unbounded::conf_height_details::get_hash () const
{
	nano::block_hash hash;
	rsnano::rsn_conf_height_details_hash (handle, hash.bytes.data ());
	return hash;
}
uint64_t nano::confirmation_height_unbounded::conf_height_details::get_height () const
{
	return rsnano::rsn_conf_height_details_height (handle);
}
uint64_t nano::confirmation_height_unbounded::conf_height_details::get_num_blocks_confirmed () const
{
	return rsnano::rsn_conf_height_details_num_blocks_confirmed (handle);
}
std::vector<nano::block_hash> nano::confirmation_height_unbounded::conf_height_details::get_block_callback_data () const
{
	std::vector<nano::block_hash> result;
	rsnano::U256ArrayDto dto;
	rsnano::rsn_conf_height_details_block_callback_data (handle, &dto);
	for (int i = 0; i < dto.count; ++i)
	{
		nano::block_hash hash;
		std::copy (std::begin (dto.items[i]), std::end (dto.items[i]), std::begin (hash.bytes));
		result.push_back (hash);
	}
	rsnano::rsn_u256_array_destroy (&dto);

	return result;
}
void nano::confirmation_height_unbounded::conf_height_details::set_block_callback_data (std::vector<nano::block_hash> const & data_a)
{
	std::vector<uint8_t const *> tmp;
	for (const auto & i : data_a)
	{
		tmp.push_back (i.bytes.data ());
	}
	rsnano::rsn_conf_height_details_set_block_callback_data (handle, tmp.data (), tmp.size ());
}
void nano::confirmation_height_unbounded::conf_height_details::add_block_callback_data (nano::block_hash const & hash)
{
	rsnano::rsn_conf_height_details_add_block_callback_data (handle, hash.bytes.data ());
}

nano::confirmation_height_unbounded::receive_source_pair::receive_source_pair (conf_height_details_shared_ptr const & receive_details_a, const block_hash & source_a) :
	receive_details (receive_details_a),
	source_hash (source_a)
{
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (confirmation_height_unbounded & confirmation_height_unbounded, std::string const & name_a)
{
	auto composite = std::make_unique<container_info_composite> (name_a);
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "confirmed_iterated_pairs", rsnano::rsn_conf_height_unbounded_conf_iterated_pairs_len (confirmation_height_unbounded.handle), rsnano::rsn_conf_iterated_pair_size () }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "pending_writes", rsnano::rsn_conf_height_unbounded_pending_writes_len (confirmation_height_unbounded.handle), rsnano::rsn_conf_height_details_size () }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "implicit_receive_cemented_mapping", confirmation_height_unbounded.implicit_receive_cemented_mapping_size, sizeof (decltype (confirmation_height_unbounded.implicit_receive_cemented_mapping)::value_type) }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "block_cache", confirmation_height_unbounded.block_cache_size (), sizeof (decltype (confirmation_height_unbounded.block_cache)::value_type) }));
	return composite;
}
