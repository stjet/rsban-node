#pragma once

#include "nano/lib/rsnano.hpp"

#include <nano/lib/numbers.hpp>
#include <nano/lib/rsnanoutils.hpp>
#include <nano/lib/threading.hpp>
#include <nano/lib/timer.hpp>
#include <nano/secure/store.hpp>

#include <cstddef>
#include <cstdint>
#include <optional>

namespace nano
{
class ledger;
class read_transaction;
class logging;
class logger_mt;
class write_database_queue;
class write_guard;

class hash_circular_buffer
{
public:
	hash_circular_buffer (size_t max_items);
	hash_circular_buffer (hash_circular_buffer const &) = delete;
	hash_circular_buffer (hash_circular_buffer &&) = delete;
	~hash_circular_buffer ();

	bool empty () const;
	nano::block_hash back () const;
	void push_back (nano::block_hash const &);
	void truncate_after (nano::block_hash const &);

	rsnano::HashCircularBufferHandle * handle;
};

class confirmation_height_bounded final
{
public:
	confirmation_height_bounded (nano::ledger &, nano::write_database_queue &, std::chrono::milliseconds batch_separate_pending_min_time, nano::logging const &, std::shared_ptr<nano::logger_mt> &, std::atomic<bool> & stopped, rsnano::AtomicU64Wrapper & batch_write_size, std::function<void (std::vector<std::shared_ptr<nano::block>> const &)> const & cemented_callback, std::function<void (nano::block_hash const &)> const & already_cemented_callback, std::function<uint64_t ()> const & awaiting_processing_size_query);
	confirmation_height_bounded (confirmation_height_bounded const &) = delete;
	confirmation_height_bounded (confirmation_height_bounded &&) = delete;
	~confirmation_height_bounded ();
	bool pending_empty () const;
	void clear_process_vars ();
	void process (std::shared_ptr<nano::block> original_block);
	void cement_blocks (nano::write_guard & scoped_write_guard_a);

private:
	class top_and_next_hash final
	{
	public:
		top_and_next_hash () = default;
		top_and_next_hash (nano::block_hash top, boost::optional<nano::block_hash> next, uint64_t next_height) :
			top{ top },
			next{ next },
			next_height{ next_height }
		{
		}
		top_and_next_hash (rsnano::TopAndNextHashDto const & dto)
		{
			top = nano::block_hash::from_bytes (dto.top);
			if (dto.has_next)
			{
				next = nano::block_hash::from_bytes (dto.next);
			}
			next_height = dto.next_height;
		}
		nano::block_hash top;
		boost::optional<nano::block_hash> next;
		uint64_t next_height;

		rsnano::TopAndNextHashDto to_dto () const
		{
			rsnano::TopAndNextHashDto dto;
			std::copy (std::begin (top.bytes), std::end (top.bytes), std::begin (dto.top));
			dto.has_next = next.has_value ();
			if (next)
			{
				std::copy (std::begin (next->bytes), std::end (next->bytes), std::begin (dto.next));
			}
			dto.next_height = next_height;
			return dto;
		}
	};

	class confirmed_info
	{
	public:
		confirmed_info (uint64_t confirmed_height_a, nano::block_hash const & iterated_frontier);
		uint64_t confirmed_height;
		nano::block_hash iterated_frontier;
	};

	class write_details final
	{
	public:
		write_details (nano::account const &, uint64_t, nano::block_hash const &, uint64_t, nano::block_hash const &);
		write_details (rsnano::WriteDetailsDto const & dto);
		nano::account account;
		// This is the first block hash (bottom most) which is not cemented
		uint64_t bottom_height;
		nano::block_hash bottom_hash;
		// Desired cemented frontier
		uint64_t top_height;
		nano::block_hash top_hash;

		rsnano::WriteDetailsDto to_dto () const;
	};

	class pending_writes_queue
	{
	public:
		pending_writes_queue (rsnano::ConfirmationHeightBoundedHandle * handle);
		size_t size () const;
		bool empty () const;
		void push_back (write_details const & details);
		write_details front () const;
		void pop_front ();
		uint64_t total_pending_write_block_count () const;
		rsnano::ConfirmationHeightBoundedHandle * handle;
	};

	class accounts_confirmed_info_map
	{
	public:
		accounts_confirmed_info_map (rsnano::ConfirmationHeightBoundedHandle * handle_a) :
			handle{ handle_a }
		{
		}

		std::optional<confirmed_info> find (nano::account const & account)
		{
			rsnano::ConfirmedInfoDto result;
			if (rsnano::rsn_accounts_confirmed_info_find (handle, account.bytes.data (), &result))
			{
				nano::block_hash hash;
				std::copy (std::begin (result.iterated_frontier), std::end (result.iterated_frontier), std::begin (hash.bytes));
				return confirmed_info{ result.confirmed_height, hash };
			}
			else
			{
				return std::nullopt;
			}
		}

		size_t size () const
		{
			return rsnano::rsn_accounts_confirmed_info_size (handle);
		}

		void insert (nano::account const & account, confirmed_info const & info)
		{
			rsnano::ConfirmedInfoDto info_dto;
			info_dto.confirmed_height = info.confirmed_height;
			std::copy (std::begin (info.iterated_frontier.bytes), std::end (info.iterated_frontier.bytes), std::begin (info_dto.iterated_frontier));
			rsnano::rsn_accounts_confirmed_info_insert (handle, account.bytes.data (), &info_dto);
		}

		void erase (nano::account const & account)
		{
			rsnano::rsn_accounts_confirmed_info_erase (handle, account.bytes.data ());
		}

		void clear ()
		{
			rsnano::rsn_accounts_confirmed_info_clear (handle);
		}

		rsnano::ConfirmationHeightBoundedHandle * handle;
	};

	/** The maximum number of blocks to be read in while iterating over a long account chain */
	uint64_t const batch_read_size = 65536;

	/** The maximum number of various containers to keep the memory bounded */
	uint32_t const max_items{ 131072 };

	rsnano::ConfirmationHeightBoundedHandle * handle;

	// All of the atomic variables here just track the size for use in collect_container_info.
	// This is so that no mutexes are needed during the algorithm itself, which would otherwise be needed
	// for the sake of a rarely used RPC call for debugging purposes. As such the sizes are not being acted
	// upon in any way (does not synchronize with any other data).
	// This allows the load and stores to use relaxed atomic memory ordering.
	pending_writes_queue pending_writes;
	uint32_t const pending_writes_max_size{ max_items };
	/* Holds confirmation height/cemented frontier in memory for accounts while iterating */
	accounts_confirmed_info_map accounts_confirmed_info;

	class receive_chain_details final
	{
	public:
		receive_chain_details (nano::account const &, uint64_t, nano::block_hash const &, nano::block_hash const &, boost::optional<nano::block_hash>, uint64_t, nano::block_hash const &);
		receive_chain_details (rsnano::ReceiveChainDetailsDto const &);
		nano::account account;
		uint64_t height;
		nano::block_hash hash;
		nano::block_hash top_level;
		boost::optional<nano::block_hash> next;
		uint64_t bottom_height;
		nano::block_hash bottom_most;

		rsnano::ReceiveChainDetailsDto to_dto () const;
	};

	class preparation_data final
	{
	public:
		nano::transaction const & transaction;
		nano::block_hash const & top_most_non_receive_block_hash;
		bool already_cemented;
		nano::hash_circular_buffer & checkpoints;
		nano::confirmation_height_info const & confirmation_height_info;
		nano::account const & account;
		uint64_t bottom_height;
		nano::block_hash const & bottom_most;
		boost::optional<receive_chain_details> & receive_details;
		boost::optional<top_and_next_hash> next_in_receive_chain;
	};

	class receive_source_pair final
	{
	public:
		receive_source_pair (receive_chain_details const &, nano::block_hash const &);
		receive_source_pair (rsnano::ReceiveSourcePairDto const &);

		receive_chain_details receive_details;
		nano::block_hash source_hash;

		rsnano::ReceiveSourcePairDto to_dto () const;
	};
	class receive_source_pair_circular_buffer
	{
	public:
		receive_source_pair_circular_buffer (size_t max_items);
		receive_source_pair_circular_buffer (receive_source_pair_circular_buffer const &) = delete;
		receive_source_pair_circular_buffer (receive_source_pair_circular_buffer &&) = delete;
		~receive_source_pair_circular_buffer ();

		void push_back (receive_source_pair const &);
		bool empty () const;
		size_t size () const;
		receive_source_pair back () const;
		void pop_back ();

		rsnano::ReceiveSourcePairCircularBufferHandle * handle;
	};

	nano::timer<std::chrono::milliseconds> timer;

	top_and_next_hash get_next_block (boost::optional<top_and_next_hash> const &, nano::hash_circular_buffer const &, receive_source_pair_circular_buffer const & receive_source_pairs, boost::optional<receive_chain_details> &, nano::block const & original_block);
	nano::block_hash get_least_unconfirmed_hash_from_top_level (nano::transaction const &, nano::block_hash const &, nano::account const &, nano::confirmation_height_info const &, uint64_t &);
	boost::optional<top_and_next_hash> prepare_iterated_blocks_for_cementing (preparation_data &);

	bool iterate (
	nano::read_transaction &,
	uint64_t,
	nano::block_hash const &,
	nano::hash_circular_buffer &,
	nano::block_hash &,
	nano::block_hash const &,
	receive_source_pair_circular_buffer &,
	nano::account const &);

	nano::ledger & ledger;
	nano::write_database_queue & write_database_queue;
	std::chrono::milliseconds batch_separate_pending_min_time;
	nano::logging const & logging;
	std::shared_ptr<nano::logger_mt> & logger;
	std::atomic<bool> & stopped;
	rsnano::AtomicU64Wrapper & batch_write_size;
	std::function<void (std::vector<std::shared_ptr<nano::block>> const &)> notify_observers_callback;
	std::function<void (nano::block_hash const &)> notify_block_already_cemented_observers_callback;
	std::function<uint64_t ()> awaiting_processing_size_callback;

	friend std::unique_ptr<nano::container_info_component> collect_container_info (confirmation_height_bounded &, std::string const & name_a);
};

std::unique_ptr<nano::container_info_component> collect_container_info (confirmation_height_bounded &, std::string const & name_a);
}
