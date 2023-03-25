#pragma once

#include "nano/lib/rsnano.hpp"

#include <nano/lib/numbers.hpp>
#include <nano/lib/rsnanoutils.hpp>

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
	rsnano::ConfirmationHeightBoundedHandle * handle;
};
}
