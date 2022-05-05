#include <nano/lib/logger_mt.hpp>
#include <nano/lib/numbers.hpp>
#include <nano/lib/rsnano.hpp>
#include <nano/lib/threading.hpp>
#include <nano/lib/timer.hpp>
#include <nano/node/nodeconfig.hpp>
#include <nano/node/signatures.hpp>
#include <nano/node/state_block_signature_verification.hpp>
#include <nano/secure/common.hpp>

#include <boost/format.hpp>

void item_to_dto (nano::state_block_signature_verification::value_type const & value, rsnano::StateBlockSignatureVerificationValueDto & result)
{
	auto const & [block, account, verification] = value;
	result.block = block->clone_handle ();
	std::copy (std::begin (account.bytes), std::end (account.bytes), std::begin (result.account));
	result.verification = static_cast<uint8_t> (verification);
}

std::vector<rsnano::StateBlockSignatureVerificationValueDto> items_to_dto (std::deque<nano::state_block_signature_verification::value_type> & items)
{
	std::vector<rsnano::StateBlockSignatureVerificationValueDto> result;
	result.reserve (items.size ());
	for (auto i : items)
	{
		rsnano::StateBlockSignatureVerificationValueDto value_dto;
		item_to_dto (i, value_dto);
		result.push_back (value_dto);
	}

	return result;
}

void dto_to_value_type (rsnano::StateBlockSignatureVerificationValueDto const & dto, nano::state_block_signature_verification::value_type & result)
{
	nano::account account;
	std::copy (std::begin (dto.account), std::end (dto.account), std::begin (account.bytes));
	auto verification = static_cast<nano::signature_verification> (dto.verification);
	result = nano::state_block_signature_verification::value_type (nano::block_handle_to_block (dto.block), account, verification);
}

void blocks_verified_callback_adapter (void * context, const rsnano::StateBlockSignatureVerificationResultDto * result_dto)
{
	std::vector<int> verifications (result_dto->verifications, result_dto->verifications + result_dto->size);
	std::vector<nano::block_hash> hashes;
	for (auto i = result_dto->hashes; i != result_dto->hashes + result_dto->size; ++i)
	{
		nano::block_hash hash;
		std::copy (std::begin (*i), std::end (*i), std::begin (hash.bytes));
		hashes.push_back (hash);
	}

	std::vector<nano::signature> blocks_signatures;
	blocks_signatures.reserve (result_dto->size);
	for (auto i = result_dto->signatures; i != result_dto->signatures + result_dto->size; ++i)
	{
		nano::signature signature;
		std::copy (std::begin (*i), std::end (*i), std::begin (signature.bytes));
		blocks_signatures.push_back (signature);
	}

	std::deque<nano::state_block_signature_verification::value_type> items;
	for (auto i = result_dto->items; i != result_dto->items + result_dto->size; i++)
	{
		nano::state_block_signature_verification::value_type value;
		dto_to_value_type (*i, value);
		items.push_back (value);
	}

	auto instance = reinterpret_cast<nano::state_block_signature_verification *> (context);
	instance->blocks_verified_callback (items, verifications, hashes, blocks_signatures);
}

nano::state_block_signature_verification::state_block_signature_verification (nano::signature_checker & signature_checker, nano::epochs & epochs, nano::node_config & node_config, nano::logger_mt & logger, uint64_t state_block_signature_verification_size) :
	epochs (epochs),
	node_config (node_config)
{
	handle = rsnano::rsn_state_block_signature_verification_create (signature_checker.get_handle (), epochs.get_handle (), &logger, node_config.logging.timing_logging ());
	rsnano::rsn_state_block_signature_verification_verified_callback (handle, blocks_verified_callback_adapter, this);
	thread = std::thread ([this, state_block_signature_verification_size] () {
		nano::thread_role::set (nano::thread_role::name::state_block_signature_verification);
		this->run (state_block_signature_verification_size);
	});
}

nano::state_block_signature_verification::~state_block_signature_verification ()
{
	stop ();
	rsnano::rsn_state_block_signature_verification_destroy (handle);
}

void nano::state_block_signature_verification::stop ()
{
	{
		nano::lock_guard<nano::mutex> guard (mutex);
		rsnano::rsn_state_block_signature_verification_set_stopped (handle, true);
	}

	if (thread.joinable ())
	{
		condition.notify_one ();
		thread.join ();
	}
}

void nano::state_block_signature_verification::run (uint64_t state_block_signature_verification_size)
{
	nano::unique_lock<nano::mutex> lk (mutex);
	while (!rsnano::rsn_state_block_signature_verification_get_stopped (handle))
	{
		if (!rsnano::rsn_state_block_signature_verification_blocks_empty (handle))
		{
			std::size_t const max_verification_batch (state_block_signature_verification_size != 0 ? state_block_signature_verification_size : nano::signature_checker::get_batch_size () * (node_config.signature_checker_threads + 1));
			rsnano::rsn_state_block_signature_verification_set_active (handle, true);
			while (!rsnano::rsn_state_block_signature_verification_blocks_empty (handle) && !rsnano::rsn_state_block_signature_verification_get_stopped (handle))
			{
				auto items = setup_items (max_verification_batch);
				lk.unlock ();

				auto item_dtos (items_to_dto (items));
				rsnano::rsn_state_block_signature_verification_verify (handle, item_dtos.data (), item_dtos.size ());
				for (auto & i : item_dtos)
				{
					rsnano::rsn_shared_block_enum_handle_destroy (i.block);
				}

				lk.lock ();
			}
			rsnano::rsn_state_block_signature_verification_set_active (handle, false);
			lk.unlock ();
			transition_inactive_callback ();
			lk.lock ();
		}
		else
		{
			condition.wait (lk);
		}
	}
}

bool nano::state_block_signature_verification::is_active ()
{
	nano::lock_guard<nano::mutex> guard (mutex);
	return rsnano::rsn_state_block_signature_verification_get_active (handle);
}

void nano::state_block_signature_verification::add (value_type const & item)
{
	{
		nano::lock_guard<nano::mutex> guard (mutex);
		rsnano::StateBlockSignatureVerificationValueDto dto;
		item_to_dto (item, dto);
		rsnano::rsn_state_block_signature_verification_blocks_push (handle, &dto);
	}
	condition.notify_one ();
}

std::size_t nano::state_block_signature_verification::size ()
{
	nano::lock_guard<nano::mutex> guard (mutex);
	return rsnano::rsn_state_block_signature_verification_size (handle);
}

auto nano::state_block_signature_verification::setup_items (std::size_t max_count) -> std::deque<value_type>
{
	std::deque<value_type> items;
	std::vector<rsnano::StateBlockSignatureVerificationValueDto> item_dtos;
	item_dtos.resize (max_count);
	auto count = rsnano::rsn_state_block_signature_verification_setup_items (handle, max_count, item_dtos.data ());
	for (auto i (0); i < count; i++)
	{
		nano::state_block_signature_verification::value_type item;
		dto_to_value_type (item_dtos[i], item);
		items.push_back (item);
	}

	return items;
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (state_block_signature_verification & state_block_signature_verification, std::string const & name)
{
	auto composite = std::make_unique<container_info_composite> (name);
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "state_blocks", state_block_signature_verification.size (), sizeof (state_block_signature_verification::value_type) }));
	return composite;
}