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
	confirmation_height_bounded (nano::ledger &, nano::write_database_queue &, std::chrono::milliseconds batch_separate_pending_min_time, nano::logging const &, std::shared_ptr<nano::logger_mt> &, rsnano::AtomicBoolWrapper & stopped, rsnano::AtomicU64Wrapper & batch_write_size, std::function<void (std::vector<std::shared_ptr<nano::block>> const &)> const & cemented_callback, std::function<void (nano::block_hash const &)> const & already_cemented_callback, std::function<uint64_t ()> const & awaiting_processing_size_query);
	confirmation_height_bounded (confirmation_height_bounded const &) = delete;
	confirmation_height_bounded (confirmation_height_bounded &&) = delete;
	~confirmation_height_bounded ();
	bool pending_empty () const;
	void clear_process_vars ();
	void process (std::shared_ptr<nano::block> original_block);
	void cement_blocks (nano::write_guard & scoped_write_guard_a);

private:
	rsnano::ConfirmationHeightBoundedHandle * handle;

	nano::ledger & ledger;
	nano::write_database_queue & write_database_queue;
	std::chrono::milliseconds batch_separate_pending_min_time;
	nano::logging const & logging;
	std::shared_ptr<nano::logger_mt> & logger;
	rsnano::AtomicBoolWrapper & stopped;
	rsnano::AtomicU64Wrapper & batch_write_size;
	std::function<void (std::vector<std::shared_ptr<nano::block>> const &)> notify_observers_callback;
	std::function<void (nano::block_hash const &)> notify_block_already_cemented_observers_callback;
	std::function<uint64_t ()> awaiting_processing_size_callback;

	friend std::unique_ptr<nano::container_info_component> collect_container_info (confirmation_height_bounded &, std::string const & name_a);
};

std::unique_ptr<nano::container_info_component> collect_container_info (confirmation_height_bounded &, std::string const & name_a);
}
