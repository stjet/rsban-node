#include <nano/lib/rep_weights.hpp>
#include <nano/lib/rsnanoutils.hpp>
#include <nano/lib/stats.hpp>
#include <nano/lib/utility.hpp>
#include <nano/lib/work.hpp>
#include <nano/secure/common.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/secure/store.hpp>

#include <boost/multiprecision/cpp_int.hpp>

namespace
{
class ledger_processor : public nano::mutable_block_visitor
{
public:
	ledger_processor (nano::ledger &, nano::stat &, nano::ledger_constants &, nano::write_transaction const &, nano::signature_verification = nano::signature_verification::unknown);
	virtual ~ledger_processor () = default;
	void send_block (nano::send_block &) override;
	void receive_block (nano::receive_block &) override;
	void open_block (nano::open_block &) override;
	void change_block (nano::change_block &) override;
	void state_block (nano::state_block &) override;
	void state_block_impl (nano::state_block &);
	void epoch_block_impl (nano::state_block &);
	nano::ledger & ledger;
	nano::stat & stats;
	nano::ledger_constants & constants;
	nano::write_transaction const & transaction;
	nano::signature_verification verification;
	nano::process_return result;

private:
	bool validate_epoch_block (nano::state_block const & block_a);
};

// Returns true if this block which has an epoch link is correctly formed.
bool ledger_processor::validate_epoch_block (nano::state_block const & block_a)
{
	debug_assert (ledger.is_epoch_link (block_a.link ()));
	nano::amount prev_balance (0);
	if (!block_a.previous ().is_zero ())
	{
		result.code = ledger.store.block ().exists (transaction, block_a.previous ()) ? nano::process_result::progress : nano::process_result::gap_previous;
		if (result.code == nano::process_result::progress)
		{
			prev_balance = ledger.balance (transaction, block_a.previous ());
		}
		else if (result.verified == nano::signature_verification::unknown)
		{
			// Check for possible regular state blocks with epoch link (send subtype)
			if (validate_message (block_a.account (), block_a.hash (), block_a.block_signature ()))
			{
				// Is epoch block signed correctly
				if (validate_message (ledger.epoch_signer (block_a.link ()), block_a.hash (), block_a.block_signature ()))
				{
					result.verified = nano::signature_verification::invalid;
					result.code = nano::process_result::bad_signature;
				}
				else
				{
					result.verified = nano::signature_verification::valid_epoch;
				}
			}
			else
			{
				result.verified = nano::signature_verification::valid;
			}
		}
	}
	return (block_a.balance () == prev_balance);
}

void ledger_processor::state_block (nano::state_block & block_a)
{
	result.code = nano::process_result::progress;
	auto is_epoch_block = false;
	if (ledger.is_epoch_link (block_a.link ()))
	{
		// This function also modifies the result variable if epoch is mal-formed
		is_epoch_block = validate_epoch_block (block_a);
	}

	if (result.code == nano::process_result::progress)
	{
		if (is_epoch_block)
		{
			epoch_block_impl (block_a);
		}
		else
		{
			state_block_impl (block_a);
		}
	}
}

void ledger_processor::state_block_impl (nano::state_block & block_a)
{
	auto hash (block_a.hash ());
	auto existing (ledger.block_or_pruned_exists (transaction, hash));
	result.code = existing ? nano::process_result::old : nano::process_result::progress; // Have we seen this block before? (Unambiguous)
	if (result.code == nano::process_result::progress)
	{
		// Validate block if not verified outside of ledger
		if (result.verified != nano::signature_verification::valid)
		{
			result.code = validate_message (block_a.account (), hash, block_a.block_signature ()) ? nano::process_result::bad_signature : nano::process_result::progress; // Is this block signed correctly (Unambiguous)
		}
		if (result.code == nano::process_result::progress)
		{
			debug_assert (!validate_message (block_a.account (), hash, block_a.block_signature ()));
			result.verified = nano::signature_verification::valid;
			result.code = block_a.account ().is_zero () ? nano::process_result::opened_burn_account : nano::process_result::progress; // Is this for the burn account? (Unambiguous)
			if (result.code == nano::process_result::progress)
			{
				nano::epoch epoch (nano::epoch::epoch_0);
				nano::epoch source_epoch (nano::epoch::epoch_0);
				nano::account_info info;
				nano::amount amount (block_a.balance ());
				auto is_send (false);
				auto is_receive (false);
				auto account_error (ledger.store.account ().get (transaction, block_a.account (), info));
				if (!account_error)
				{
					// Account already exists
					epoch = info.epoch ();
					result.previous_balance = info.balance ();
					result.code = block_a.previous ().is_zero () ? nano::process_result::fork : nano::process_result::progress; // Has this account already been opened? (Ambigious)
					if (result.code == nano::process_result::progress)
					{
						result.code = ledger.store.block ().exists (transaction, block_a.previous ()) ? nano::process_result::progress : nano::process_result::gap_previous; // Does the previous block exist in the ledger? (Unambigious)
						if (result.code == nano::process_result::progress)
						{
							is_send = block_a.balance () < info.balance ();
							is_receive = !is_send && !block_a.link ().is_zero ();
							amount = is_send ? (info.balance ().number () - amount.number ()) : (amount.number () - info.balance ().number ());
							result.code = block_a.previous () == info.head () ? nano::process_result::progress : nano::process_result::fork; // Is the previous block the account's head block? (Ambigious)
						}
					}
				}
				else
				{
					// Account does not yet exists
					result.previous_balance = 0;
					result.code = block_a.previous ().is_zero () ? nano::process_result::progress : nano::process_result::gap_previous; // Does the first block in an account yield 0 for previous() ? (Unambigious)
					if (result.code == nano::process_result::progress)
					{
						is_receive = true;
						result.code = !block_a.link ().is_zero () ? nano::process_result::progress : nano::process_result::gap_source; // Is the first block receiving from a send ? (Unambigious)
					}
				}
				if (result.code == nano::process_result::progress)
				{
					if (!is_send)
					{
						if (!block_a.link ().is_zero ())
						{
							result.code = ledger.block_or_pruned_exists (transaction, block_a.link ().as_block_hash ()) ? nano::process_result::progress : nano::process_result::gap_source; // Have we seen the source block already? (Harmless)
							if (result.code == nano::process_result::progress)
							{
								nano::pending_key key (block_a.account (), block_a.link ().as_block_hash ());
								nano::pending_info pending;
								result.code = ledger.store.pending ().get (transaction, key, pending) ? nano::process_result::unreceivable : nano::process_result::progress; // Has this source already been received (Malformed)
								if (result.code == nano::process_result::progress)
								{
									result.code = amount == pending.amount ? nano::process_result::progress : nano::process_result::balance_mismatch;
									source_epoch = pending.epoch;
									epoch = std::max (epoch, source_epoch);
								}
							}
						}
						else
						{
							// If there's no link, the balance must remain the same, only the representative can change
							result.code = amount.is_zero () ? nano::process_result::progress : nano::process_result::balance_mismatch;
						}
					}
				}
				if (result.code == nano::process_result::progress)
				{
					nano::block_details block_details (epoch, is_send, is_receive, false);
					result.code = constants.work.difficulty (block_a) >= constants.work.threshold (block_a.work_version (), block_details) ? nano::process_result::progress : nano::process_result::insufficient_work; // Does this block have sufficient work? (Malformed)
					if (result.code == nano::process_result::progress)
					{
						stats.inc (nano::stat::type::ledger, nano::stat::detail::state_block);
						block_a.sideband_set (nano::block_sideband (block_a.account () /* unused */, 0, 0 /* unused */, info.block_count () + 1, nano::seconds_since_epoch (), block_details, source_epoch));
						ledger.store.block ().put (transaction, hash, block_a);

						if (!info.head ().is_zero ())
						{
							// Move existing representation & add in amount delta
							ledger.cache.rep_weights ().representation_add_dual (info.representative (), 0 - info.balance ().number (), block_a.representative (), block_a.balance ().number ());
						}
						else
						{
							// Add in amount delta only
							ledger.cache.rep_weights ().representation_add (block_a.representative (), block_a.balance ().number ());
						}

						if (is_send)
						{
							nano::pending_key key (block_a.link ().as_account (), hash);
							nano::pending_info info (block_a.account (), amount.number (), epoch);
							ledger.store.pending ().put (transaction, key, info);
						}
						else if (!block_a.link ().is_zero ())
						{
							ledger.store.pending ().del (transaction, nano::pending_key (block_a.account (), block_a.link ().as_block_hash ()));
						}

						nano::account_info new_info (hash, block_a.representative (), info.open_block ().is_zero () ? hash : info.open_block (), block_a.balance (), nano::seconds_since_epoch (), info.block_count () + 1, epoch);
						ledger.update_account (transaction, block_a.account (), info, new_info);
						if (!ledger.store.frontier ().get (transaction, info.head ()).is_zero ())
						{
							ledger.store.frontier ().del (transaction, info.head ());
						}
					}
				}
			}
		}
	}
}

void ledger_processor::epoch_block_impl (nano::state_block & block_a)
{
	auto hash (block_a.hash ());
	auto existing (ledger.block_or_pruned_exists (transaction, hash));
	result.code = existing ? nano::process_result::old : nano::process_result::progress; // Have we seen this block before? (Unambiguous)
	if (result.code == nano::process_result::progress)
	{
		// Validate block if not verified outside of ledger
		if (result.verified != nano::signature_verification::valid_epoch)
		{
			result.code = validate_message (ledger.epoch_signer (block_a.link ()), hash, block_a.block_signature ()) ? nano::process_result::bad_signature : nano::process_result::progress; // Is this block signed correctly (Unambiguous)
		}
		if (result.code == nano::process_result::progress)
		{
			debug_assert (!validate_message (ledger.epoch_signer (block_a.link ()), hash, block_a.block_signature ()));
			result.verified = nano::signature_verification::valid_epoch;
			result.code = block_a.account ().is_zero () ? nano::process_result::opened_burn_account : nano::process_result::progress; // Is this for the burn account? (Unambiguous)
			if (result.code == nano::process_result::progress)
			{
				nano::account_info info;
				auto account_error (ledger.store.account ().get (transaction, block_a.account (), info));
				if (!account_error)
				{
					// Account already exists
					result.previous_balance = info.balance ();
					result.code = block_a.previous ().is_zero () ? nano::process_result::fork : nano::process_result::progress; // Has this account already been opened? (Ambigious)
					if (result.code == nano::process_result::progress)
					{
						result.code = block_a.previous () == info.head () ? nano::process_result::progress : nano::process_result::fork; // Is the previous block the account's head block? (Ambigious)
						if (result.code == nano::process_result::progress)
						{
							result.code = block_a.representative () == info.representative () ? nano::process_result::progress : nano::process_result::representative_mismatch;
						}
					}
				}
				else
				{
					result.previous_balance = 0;
					result.code = block_a.representative ().is_zero () ? nano::process_result::progress : nano::process_result::representative_mismatch;
					// Non-exisitng account should have pending entries
					if (result.code == nano::process_result::progress)
					{
						bool pending_exists = ledger.store.pending ().any (transaction, block_a.account ());
						result.code = pending_exists ? nano::process_result::progress : nano::process_result::gap_epoch_open_pending;
					}
				}
				if (result.code == nano::process_result::progress)
				{
					auto epoch = constants.epochs.epoch (block_a.link ());
					// Must be an epoch for an unopened account or the epoch upgrade must be sequential
					auto is_valid_epoch_upgrade = account_error ? static_cast<std::underlying_type_t<nano::epoch>> (epoch) > 0 : nano::epochs::is_sequential (info.epoch (), epoch);
					result.code = is_valid_epoch_upgrade ? nano::process_result::progress : nano::process_result::block_position;
					if (result.code == nano::process_result::progress)
					{
						result.code = block_a.balance () == info.balance () ? nano::process_result::progress : nano::process_result::balance_mismatch;
						if (result.code == nano::process_result::progress)
						{
							nano::block_details block_details (epoch, false, false, true);
							result.code = constants.work.difficulty (block_a) >= constants.work.threshold (block_a.work_version (), block_details) ? nano::process_result::progress : nano::process_result::insufficient_work; // Does this block have sufficient work? (Malformed)
							if (result.code == nano::process_result::progress)
							{
								stats.inc (nano::stat::type::ledger, nano::stat::detail::epoch_block);
								block_a.sideband_set (nano::block_sideband (block_a.account () /* unused */, 0, 0 /* unused */, info.block_count () + 1, nano::seconds_since_epoch (), block_details, nano::epoch::epoch_0 /* unused */));
								ledger.store.block ().put (transaction, hash, block_a);
								nano::account_info new_info (hash, block_a.representative (), info.open_block ().is_zero () ? hash : info.open_block (), info.balance (), nano::seconds_since_epoch (), info.block_count () + 1, epoch);
								ledger.update_account (transaction, block_a.account (), info, new_info);
								if (!ledger.store.frontier ().get (transaction, info.head ()).is_zero ())
								{
									ledger.store.frontier ().del (transaction, info.head ());
								}
							}
						}
					}
				}
			}
		}
	}
}

void ledger_processor::change_block (nano::change_block & block_a)
{
	auto hash (block_a.hash ());
	auto existing (ledger.block_or_pruned_exists (transaction, hash));
	result.code = existing ? nano::process_result::old : nano::process_result::progress; // Have we seen this block before? (Harmless)
	if (result.code == nano::process_result::progress)
	{
		auto previous (ledger.store.block ().get (transaction, block_a.previous ()));
		result.code = previous != nullptr ? nano::process_result::progress : nano::process_result::gap_previous; // Have we seen the previous block already? (Harmless)
		if (result.code == nano::process_result::progress)
		{
			result.code = block_a.valid_predecessor (*previous) ? nano::process_result::progress : nano::process_result::block_position;
			if (result.code == nano::process_result::progress)
			{
				auto account (ledger.store.frontier ().get (transaction, block_a.previous ()));
				result.code = account.is_zero () ? nano::process_result::fork : nano::process_result::progress;
				if (result.code == nano::process_result::progress)
				{
					nano::account_info info;
					auto latest_error (ledger.store.account ().get (transaction, account, info));
					(void)latest_error;
					debug_assert (!latest_error);
					debug_assert (info.head () == block_a.previous ());
					// Validate block if not verified outside of ledger
					if (result.verified != nano::signature_verification::valid)
					{
						result.code = validate_message (account, hash, block_a.block_signature ()) ? nano::process_result::bad_signature : nano::process_result::progress; // Is this block signed correctly (Malformed)
					}
					if (result.code == nano::process_result::progress)
					{
						nano::block_details block_details (nano::epoch::epoch_0, false /* unused */, false /* unused */, false /* unused */);
						result.code = constants.work.difficulty (block_a) >= constants.work.threshold (block_a.work_version (), block_details) ? nano::process_result::progress : nano::process_result::insufficient_work; // Does this block have sufficient work? (Malformed)
						if (result.code == nano::process_result::progress)
						{
							debug_assert (!validate_message (account, hash, block_a.block_signature ()));
							result.verified = nano::signature_verification::valid;
							block_a.sideband_set (nano::block_sideband (account, 0, info.balance (), info.block_count () + 1, nano::seconds_since_epoch (), block_details, nano::epoch::epoch_0 /* unused */));
							ledger.store.block ().put (transaction, hash, block_a);
							auto balance (ledger.balance (transaction, block_a.previous ()));
							ledger.cache.rep_weights ().representation_add_dual (block_a.representative (), balance, info.representative (), 0 - balance);
							nano::account_info new_info (hash, block_a.representative (), info.open_block (), info.balance (), nano::seconds_since_epoch (), info.block_count () + 1, nano::epoch::epoch_0);
							ledger.update_account (transaction, account, info, new_info);
							ledger.store.frontier ().del (transaction, block_a.previous ());
							ledger.store.frontier ().put (transaction, hash, account);
							result.previous_balance = info.balance ();
							stats.inc (nano::stat::type::ledger, nano::stat::detail::change);
						}
					}
				}
			}
		}
	}
}

void ledger_processor::send_block (nano::send_block & block_a)
{
	auto hash (block_a.hash ());
	auto existing (ledger.block_or_pruned_exists (transaction, hash));
	result.code = existing ? nano::process_result::old : nano::process_result::progress; // Have we seen this block before? (Harmless)
	if (result.code == nano::process_result::progress)
	{
		auto previous (ledger.store.block ().get (transaction, block_a.previous ()));
		result.code = previous != nullptr ? nano::process_result::progress : nano::process_result::gap_previous; // Have we seen the previous block already? (Harmless)
		if (result.code == nano::process_result::progress)
		{
			result.code = block_a.valid_predecessor (*previous) ? nano::process_result::progress : nano::process_result::block_position;
			if (result.code == nano::process_result::progress)
			{
				auto account (ledger.store.frontier ().get (transaction, block_a.previous ()));
				result.code = account.is_zero () ? nano::process_result::fork : nano::process_result::progress;
				if (result.code == nano::process_result::progress)
				{
					// Validate block if not verified outside of ledger
					if (result.verified != nano::signature_verification::valid)
					{
						result.code = validate_message (account, hash, block_a.block_signature ()) ? nano::process_result::bad_signature : nano::process_result::progress; // Is this block signed correctly (Malformed)
					}
					if (result.code == nano::process_result::progress)
					{
						nano::block_details block_details (nano::epoch::epoch_0, false /* unused */, false /* unused */, false /* unused */);
						result.code = constants.work.difficulty (block_a) >= constants.work.threshold (block_a.work_version (), block_details) ? nano::process_result::progress : nano::process_result::insufficient_work; // Does this block have sufficient work? (Malformed)
						if (result.code == nano::process_result::progress)
						{
							debug_assert (!validate_message (account, hash, block_a.block_signature ()));
							result.verified = nano::signature_verification::valid;
							nano::account_info info;
							auto latest_error (ledger.store.account ().get (transaction, account, info));
							(void)latest_error;
							debug_assert (!latest_error);
							debug_assert (info.head () == block_a.previous ());
							result.code = info.balance ().number () >= block_a.balance ().number () ? nano::process_result::progress : nano::process_result::negative_spend; // Is this trying to spend a negative amount (Malicious)
							if (result.code == nano::process_result::progress)
							{
								auto amount (info.balance ().number () - block_a.balance ().number ());
								ledger.cache.rep_weights ().representation_add (info.representative (), 0 - amount);
								block_a.sideband_set (nano::block_sideband (account, 0, block_a.balance () /* unused */, info.block_count () + 1, nano::seconds_since_epoch (), block_details, nano::epoch::epoch_0 /* unused */));
								ledger.store.block ().put (transaction, hash, block_a);
								nano::account_info new_info (hash, info.representative (), info.open_block (), block_a.balance (), nano::seconds_since_epoch (), info.block_count () + 1, nano::epoch::epoch_0);
								ledger.update_account (transaction, account, info, new_info);
								ledger.store.pending ().put (transaction, nano::pending_key (block_a.destination (), hash), { account, amount, nano::epoch::epoch_0 });
								ledger.store.frontier ().del (transaction, block_a.previous ());
								ledger.store.frontier ().put (transaction, hash, account);
								result.previous_balance = info.balance ();
								stats.inc (nano::stat::type::ledger, nano::stat::detail::send);
							}
						}
					}
				}
			}
		}
	}
}

void ledger_processor::receive_block (nano::receive_block & block_a)
{
	auto hash (block_a.hash ());
	auto existing (ledger.block_or_pruned_exists (transaction, hash));
	result.code = existing ? nano::process_result::old : nano::process_result::progress; // Have we seen this block already?  (Harmless)
	if (result.code == nano::process_result::progress)
	{
		auto previous (ledger.store.block ().get (transaction, block_a.previous ()));
		result.code = previous != nullptr ? nano::process_result::progress : nano::process_result::gap_previous;
		if (result.code == nano::process_result::progress)
		{
			result.code = block_a.valid_predecessor (*previous) ? nano::process_result::progress : nano::process_result::block_position;
			if (result.code == nano::process_result::progress)
			{
				auto account (ledger.store.frontier ().get (transaction, block_a.previous ()));
				result.code = account.is_zero () ? nano::process_result::gap_previous : nano::process_result::progress; // Have we seen the previous block? No entries for account at all (Harmless)
				if (result.code == nano::process_result::progress)
				{
					// Validate block if not verified outside of ledger
					if (result.verified != nano::signature_verification::valid)
					{
						result.code = validate_message (account, hash, block_a.block_signature ()) ? nano::process_result::bad_signature : nano::process_result::progress; // Is the signature valid (Malformed)
					}
					if (result.code == nano::process_result::progress)
					{
						debug_assert (!validate_message (account, hash, block_a.block_signature ()));
						result.verified = nano::signature_verification::valid;
						result.code = ledger.block_or_pruned_exists (transaction, block_a.source ()) ? nano::process_result::progress : nano::process_result::gap_source; // Have we seen the source block already? (Harmless)
						if (result.code == nano::process_result::progress)
						{
							nano::account_info info;
							ledger.store.account ().get (transaction, account, info);
							result.code = info.head () == block_a.previous () ? nano::process_result::progress : nano::process_result::gap_previous; // Block doesn't immediately follow latest block (Harmless)
							if (result.code == nano::process_result::progress)
							{
								nano::pending_key key (account, block_a.source ());
								nano::pending_info pending;
								result.code = ledger.store.pending ().get (transaction, key, pending) ? nano::process_result::unreceivable : nano::process_result::progress; // Has this source already been received (Malformed)
								if (result.code == nano::process_result::progress)
								{
									result.code = pending.epoch == nano::epoch::epoch_0 ? nano::process_result::progress : nano::process_result::unreceivable; // Are we receiving a state-only send? (Malformed)
									if (result.code == nano::process_result::progress)
									{
										nano::block_details block_details (nano::epoch::epoch_0, false /* unused */, false /* unused */, false /* unused */);
										result.code = constants.work.difficulty (block_a) >= constants.work.threshold (block_a.work_version (), block_details) ? nano::process_result::progress : nano::process_result::insufficient_work; // Does this block have sufficient work? (Malformed)
										if (result.code == nano::process_result::progress)
										{
											auto new_balance (info.balance ().number () + pending.amount.number ());
#ifdef NDEBUG
											if (ledger.store.block ().exists (transaction, block_a.source ()))
											{
												nano::account_info source_info;
												[[maybe_unused]] auto error (ledger.store.account ().get (transaction, pending.source, source_info));
												debug_assert (!error);
											}
#endif
											ledger.store.pending ().del (transaction, key);
											block_a.sideband_set (nano::block_sideband (account, 0, new_balance, info.block_count () + 1, nano::seconds_since_epoch (), block_details, nano::epoch::epoch_0 /* unused */));
											ledger.store.block ().put (transaction, hash, block_a);
											nano::account_info new_info (hash, info.representative (), info.open_block (), new_balance, nano::seconds_since_epoch (), info.block_count () + 1, nano::epoch::epoch_0);
											ledger.update_account (transaction, account, info, new_info);
											ledger.cache.rep_weights ().representation_add (info.representative (), pending.amount.number ());
											ledger.store.frontier ().del (transaction, block_a.previous ());
											ledger.store.frontier ().put (transaction, hash, account);
											result.previous_balance = info.balance ();
											stats.inc (nano::stat::type::ledger, nano::stat::detail::receive);
										}
									}
								}
							}
						}
					}
				}
				else
				{
					result.code = ledger.store.block ().exists (transaction, block_a.previous ()) ? nano::process_result::fork : nano::process_result::gap_previous; // If we have the block but it's not the latest we have a signed fork (Malicious)
				}
			}
		}
	}
}

void ledger_processor::open_block (nano::open_block & block_a)
{
	auto hash (block_a.hash ());
	auto existing (ledger.block_or_pruned_exists (transaction, hash));
	result.code = existing ? nano::process_result::old : nano::process_result::progress; // Have we seen this block already? (Harmless)
	if (result.code == nano::process_result::progress)
	{
		// Validate block if not verified outside of ledger
		if (result.verified != nano::signature_verification::valid)
		{
			result.code = validate_message (block_a.account (), hash, block_a.block_signature ()) ? nano::process_result::bad_signature : nano::process_result::progress; // Is the signature valid (Malformed)
		}
		if (result.code == nano::process_result::progress)
		{
			debug_assert (!validate_message (block_a.account (), hash, block_a.block_signature ()));
			result.verified = nano::signature_verification::valid;
			result.code = ledger.block_or_pruned_exists (transaction, block_a.source ()) ? nano::process_result::progress : nano::process_result::gap_source; // Have we seen the source block? (Harmless)
			if (result.code == nano::process_result::progress)
			{
				nano::account_info info;
				result.code = ledger.store.account ().get (transaction, block_a.account (), info) ? nano::process_result::progress : nano::process_result::fork; // Has this account already been opened? (Malicious)
				if (result.code == nano::process_result::progress)
				{
					nano::pending_key key (block_a.account (), block_a.source ());
					nano::pending_info pending;
					result.code = ledger.store.pending ().get (transaction, key, pending) ? nano::process_result::unreceivable : nano::process_result::progress; // Has this source already been received (Malformed)
					if (result.code == nano::process_result::progress)
					{
						result.code = block_a.account () == constants.burn_account ? nano::process_result::opened_burn_account : nano::process_result::progress; // Is it burning 0 account? (Malicious)
						if (result.code == nano::process_result::progress)
						{
							result.code = pending.epoch == nano::epoch::epoch_0 ? nano::process_result::progress : nano::process_result::unreceivable; // Are we receiving a state-only send? (Malformed)
							if (result.code == nano::process_result::progress)
							{
								nano::block_details block_details (nano::epoch::epoch_0, false /* unused */, false /* unused */, false /* unused */);
								result.code = constants.work.difficulty (block_a) >= constants.work.threshold (block_a.work_version (), block_details) ? nano::process_result::progress : nano::process_result::insufficient_work; // Does this block have sufficient work? (Malformed)
								if (result.code == nano::process_result::progress)
								{
#ifdef NDEBUG
									if (ledger.store.block ().exists (transaction, block_a.source ()))
									{
										nano::account_info source_info;
										[[maybe_unused]] auto error (ledger.store.account ().get (transaction, pending.source, source_info));
										debug_assert (!error);
									}
#endif
									ledger.store.pending ().del (transaction, key);
									block_a.sideband_set (nano::block_sideband (block_a.account (), 0, pending.amount, 1, nano::seconds_since_epoch (), block_details, nano::epoch::epoch_0 /* unused */));
									ledger.store.block ().put (transaction, hash, block_a);
									nano::account_info new_info (hash, block_a.representative (), hash, pending.amount.number (), nano::seconds_since_epoch (), 1, nano::epoch::epoch_0);
									ledger.update_account (transaction, block_a.account (), info, new_info);
									ledger.cache.rep_weights ().representation_add (block_a.representative (), pending.amount.number ());
									ledger.store.frontier ().put (transaction, hash, block_a.account ());
									result.previous_balance = 0;
									stats.inc (nano::stat::type::ledger, nano::stat::detail::open);
								}
							}
						}
					}
				}
			}
		}
	}
}

ledger_processor::ledger_processor (nano::ledger & ledger_a, nano::stat & stats_a, nano::ledger_constants & constants_a, nano::write_transaction const & transaction_a, nano::signature_verification verification_a) :
	ledger (ledger_a),
	stats (stats_a),
	constants (constants_a),
	transaction (transaction_a),
	verification (verification_a)
{
	result.verified = verification;
}
} // namespace

nano::ledger::ledger (nano::store & store_a, nano::stat & stat_a, nano::ledger_constants & constants, nano::generate_cache const & generate_cache_a) :
	constants{ constants },
	store{ store_a },
	stats{ stat_a }
{
	auto constants_dto{ constants.to_dto () };
	handle = rsnano::rsn_ledger_create (store_a.get_handle (), &constants_dto, stat_a.handle, generate_cache_a.handle);
	cache = nano::ledger_cache (rsnano::rsn_ledger_get_cache_handle (handle));
}

nano::ledger::~ledger ()
{
	rsnano::rsn_ledger_destroy (handle);
}

rsnano::LedgerHandle * nano::ledger::get_handle () const
{
	return handle;
}

// Balance for account containing hash
nano::uint128_t nano::ledger::balance (nano::transaction const & transaction_a, nano::block_hash const & hash_a) const
{
	nano::amount result;
	rsnano::rsn_ledger_balance (handle, transaction_a.get_rust_handle (), hash_a.bytes.data (), result.bytes.data ());
	return result.number ();
}

nano::uint128_t nano::ledger::balance_safe (nano::transaction const & transaction_a, nano::block_hash const & hash_a, bool & error_a) const
{
	nano::amount result;
	auto success = rsnano::rsn_ledger_balance_safe (handle, transaction_a.get_rust_handle (), hash_a.bytes.data (), result.bytes.data ());
	error_a = !success;
	return result.number ();
}

// Balance for an account by account number
nano::uint128_t nano::ledger::account_balance (nano::transaction const & transaction_a, nano::account const & account_a, bool only_confirmed_a)
{
	nano::amount result;
	rsnano::rsn_ledger_account_balance (handle, transaction_a.get_rust_handle (), account_a.bytes.data (), only_confirmed_a, result.bytes.data ());
	return result.number ();
}

nano::uint128_t nano::ledger::account_receivable (nano::transaction const & transaction_a, nano::account const & account_a, bool only_confirmed_a)
{
	nano::amount result;
	rsnano::rsn_ledger_account_receivable (handle, transaction_a.get_rust_handle (), account_a.bytes.data (), only_confirmed_a, result.bytes.data ());
	return result.number ();
}

nano::process_return nano::ledger::process (nano::write_transaction const & transaction_a, nano::block & block_a, nano::signature_verification verification)
{
	debug_assert (!constants.work.validate_entry (block_a) || constants.genesis == nano::dev::genesis);
	ledger_processor processor (*this, stats, constants, transaction_a, verification);
	block_a.visit (processor);
	if (processor.result.code == nano::process_result::progress)
	{
		cache.add_blocks (1);
	}
	return processor.result;
}

nano::block_hash nano::ledger::representative (nano::transaction const & transaction_a, nano::block_hash const & hash_a)
{
	nano::block_hash result;
	rsnano::rsn_ledger_representative (handle, transaction_a.get_rust_handle (), hash_a.bytes.data (), result.bytes.data ());
	return result;
}

bool nano::ledger::block_or_pruned_exists (nano::block_hash const & hash_a) const
{
	return rsnano::rsn_ledger_block_or_pruned_exists (handle, hash_a.bytes.data ());
}

bool nano::ledger::block_or_pruned_exists (nano::transaction const & transaction_a, nano::block_hash const & hash_a) const
{
	return rsnano::rsn_ledger_block_or_pruned_exists_txn (handle, transaction_a.get_rust_handle (), hash_a.bytes.data ());
}

std::string nano::ledger::block_text (char const * hash_a)
{
	return block_text (nano::block_hash (hash_a));
}

std::string nano::ledger::block_text (nano::block_hash const & hash_a)
{
	rsnano::StringDto dto;
	rsnano::rsn_ledger_block_text (handle, hash_a.bytes.data (), &dto);
	return rsnano::convert_dto_to_string (dto);
}

bool nano::ledger::is_send (nano::transaction const & transaction_a, nano::block const & block_a) const
{
	return rsnano::rsn_ledger_is_send (handle, transaction_a.get_rust_handle (), block_a.get_handle ());
}

nano::account nano::ledger::block_destination (nano::transaction const & transaction_a, nano::block const & block_a)
{
	nano::account destination_l;
	rsnano::rsn_ledger_block_destination (handle, transaction_a.get_rust_handle (), block_a.get_handle (), destination_l.bytes.data ());
	return destination_l;
}

nano::block_hash nano::ledger::block_source (nano::transaction const & transaction_a, nano::block const & block_a)
{
	nano::block_hash source_l;
	rsnano::rsn_ledger_block_source (handle, transaction_a.get_rust_handle (), block_a.get_handle (), source_l.bytes.data ());
	return source_l;
}

std::pair<nano::block_hash, nano::block_hash> nano::ledger::hash_root_random (nano::transaction const & transaction_a) const
{
	nano::block_hash hash;
	nano::block_hash root;
	rsnano::rsn_ledger_hash_root_random (handle, transaction_a.get_rust_handle (), hash.bytes.data (), root.bytes.data ());
	return std::make_pair (hash, root);
}

// Vote weight of an account
nano::uint128_t nano::ledger::weight (nano::account const & account_a)
{
	nano::amount result;
	rsnano::rsn_ledger_weight (handle, account_a.bytes.data (), result.bytes.data ());
	return result.number ();
}

// Rollback blocks until `block_a' doesn't exist or it tries to penetrate the confirmation height
bool nano::ledger::rollback (nano::write_transaction const & transaction_a, nano::block_hash const & block_a, std::vector<std::shared_ptr<nano::block>> & list_a)
{
	rsnano::BlockArrayDto list_dto;
	auto error = rsnano::rsn_ledger_rollback (handle, transaction_a.get_rust_handle (), block_a.bytes.data (), &list_dto);
	for (int i = 0; i < list_dto.count; ++i)
	{
		list_a.push_back (nano::block_handle_to_block (list_dto.blocks[i]));
	}
	rsnano::rsn_block_array_destroy (&list_dto);
	return error;
}

bool nano::ledger::rollback (nano::write_transaction const & transaction_a, nano::block_hash const & block_a)
{
	std::vector<std::shared_ptr<nano::block>> rollback_list;
	return rollback (transaction_a, block_a, rollback_list);
}

// Return account containing hash
nano::account nano::ledger::account (nano::transaction const & transaction_a, nano::block_hash const & hash_a) const
{
	nano::account result;
	rsnano::rsn_ledger_account (handle, transaction_a.get_rust_handle (), hash_a.bytes.data (), result.bytes.data ());
	return result;
}

nano::account nano::ledger::account_safe (nano::transaction const & transaction_a, nano::block_hash const & hash_a, bool & error_a) const
{
	nano::account result;
	bool success = rsnano::rsn_ledger_account_safe (handle, transaction_a.get_rust_handle (), hash_a.bytes.data (), result.bytes.data ());
	if (!success)
	{
		error_a = true;
	}
	return result;
}

// Return amount decrease or increase for block
nano::uint128_t nano::ledger::amount (nano::transaction const & transaction_a, nano::account const & account_a)
{
	release_assert (account_a == constants.genesis->account ());
	return nano::dev::constants.genesis_amount;
}

nano::uint128_t nano::ledger::amount (nano::transaction const & transaction_a, nano::block_hash const & hash_a)
{
	nano::amount result;
	rsnano::rsn_ledger_amount (handle, transaction_a.get_rust_handle (), hash_a.bytes.data (), result.bytes.data ());
	return result.number ();
}

nano::uint128_t nano::ledger::amount_safe (nano::transaction const & transaction_a, nano::block_hash const & hash_a, bool & error_a) const
{
	nano::amount result;
	auto success = rsnano::rsn_ledger_amount_safe (handle, transaction_a.get_rust_handle (), hash_a.bytes.data (), result.bytes.data ());
	if (!success)
	{
		error_a = true;
	}
	return result.number ();
}

// Return latest block for account
nano::block_hash nano::ledger::latest (nano::transaction const & transaction_a, nano::account const & account_a)
{
	nano::block_hash latest_l;
	rsnano::rsn_ledger_latest (handle, transaction_a.get_rust_handle (), account_a.bytes.data (), latest_l.bytes.data ());
	return latest_l;
}

// Return latest root for account, account number if there are no blocks for this account.
nano::root nano::ledger::latest_root (nano::transaction const & transaction_a, nano::account const & account_a)
{
	nano::root latest_l;
	rsnano::rsn_ledger_latest_root (handle, transaction_a.get_rust_handle (), account_a.bytes.data (), latest_l.bytes.data ());
	return latest_l;
}

bool nano::ledger::could_fit (nano::transaction const & transaction_a, nano::block const & block_a) const
{
	return rsnano::rsn_ledger_could_fit (handle, transaction_a.get_rust_handle (), block_a.get_handle ());
}

bool nano::ledger::dependents_confirmed (nano::transaction const & transaction_a, nano::block const & block_a) const
{
	return rsnano::rsn_ledger_dependents_confirmed (handle, transaction_a.get_rust_handle (), block_a.get_handle ());
}

bool nano::ledger::is_epoch_link (nano::link const & link_a) const
{
	return rsnano::rsn_ledger_is_epoch_link (handle, link_a.bytes.data ());
}

std::array<nano::block_hash, 2> nano::ledger::dependent_blocks (nano::transaction const & transaction_a, nano::block const & block_a) const
{
	std::array<nano::block_hash, 2> result;
	rsnano::rsn_ledger_dependent_blocks (handle, transaction_a.get_rust_handle (), block_a.get_handle (), result[0].bytes.data (), result[1].bytes.data ());
	return result;
}

/** Given the block hash of a send block, find the associated receive block that receives that send.
 *  The send block hash is not checked in any way, it is assumed to be correct.
 * @return Return the receive block on success and null on failure
 */
std::shared_ptr<nano::block> nano::ledger::find_receive_block_by_send_hash (nano::transaction const & transaction, nano::account const & destination, nano::block_hash const & send_block_hash)
{
	auto block_handle = rsnano::rsn_ledger_find_receive_block_by_send_hash (handle, transaction.get_rust_handle (), destination.bytes.data (), send_block_hash.bytes.data ());
	return nano::block_handle_to_block (block_handle);
}

nano::account nano::ledger::epoch_signer (nano::link const & link_a) const
{
	nano::account signer;
	rsnano::rsn_ledger_epoch_signer (handle, link_a.bytes.data (), signer.bytes.data ());
	return signer;
}

nano::link nano::ledger::epoch_link (nano::epoch epoch_a) const
{
	nano::link link;
	rsnano::rsn_ledger_epoch_link (handle, static_cast<uint8_t> (epoch_a), link.bytes.data ());
	return link;
}

void nano::ledger::update_account (nano::write_transaction const & transaction_a, nano::account const & account_a, nano::account_info const & old_a, nano::account_info const & new_a)
{
	rsnano::rsn_ledger_update_account (handle, transaction_a.get_rust_handle (), account_a.bytes.data (), old_a.handle, new_a.handle);
}

std::shared_ptr<nano::block> nano::ledger::successor (nano::transaction const & transaction_a, nano::qualified_root const & root_a)
{
	auto block_handle = rsnano::rsn_ledger_successor (handle, transaction_a.get_rust_handle (), root_a.bytes.data ());
	return nano::block_handle_to_block (block_handle);
}

bool nano::ledger::block_confirmed (nano::transaction const & transaction_a, nano::block_hash const & hash_a) const
{
	return rsnano::rsn_ledger_block_confirmed (handle, transaction_a.get_rust_handle (), hash_a.bytes.data ());
}

uint64_t nano::ledger::pruning_action (nano::write_transaction & transaction_a, nano::block_hash const & hash_a, uint64_t const batch_size_a)
{
	return rsnano::rsn_ledger_pruning_action (handle, transaction_a.get_rust_handle (), hash_a.bytes.data (), batch_size_a);
}

std::multimap<uint64_t, nano::uncemented_info, std::greater<>> nano::ledger::unconfirmed_frontiers () const
{
	rsnano::UnconfirmedFrontierArrayDto array_dto;
	rsnano::rsn_ledger_unconfirmed_frontiers (handle, &array_dto);
	std::multimap<uint64_t, nano::uncemented_info, std::greater<>> result;
	for (int i = 0; i < array_dto.count; ++i)
	{
		const auto & item_dto = array_dto.items[i].info;
		nano::block_hash cemented_frontier;
		nano::block_hash frontier;
		nano::account account;
		std::copy (std::begin (item_dto.cemented_frontier), std::end (item_dto.cemented_frontier), std::begin (cemented_frontier.bytes));
		std::copy (std::begin (item_dto.frontier), std::end (item_dto.frontier), std::begin (frontier.bytes));
		std::copy (std::begin (item_dto.account), std::end (item_dto.account), std::begin (account.bytes));
		result.emplace (std::piecewise_construct, std::forward_as_tuple (array_dto.items[i].height_delta), std::forward_as_tuple (cemented_frontier, frontier, account));
	}
	rsnano::rsn_unconfirmed_frontiers_destroy (&array_dto);
	return result;
}

bool nano::ledger::bootstrap_weight_reached () const
{
	return rsnano::rsn_ledger_bootstrap_weight_reached (handle);
}

void nano::ledger::write_confirmation_height (nano::write_transaction const & transaction_a, nano::account const & account_a, uint64_t num_blocks_cemented_a, uint64_t confirmation_height_a, nano::block_hash const & confirmed_frontier_a)
{
	rsnano::rsn_ledger_write_confirmation_height (handle, transaction_a.get_rust_handle (), account_a.bytes.data (), num_blocks_cemented_a, confirmation_height_a, confirmed_frontier_a.bytes.data ());
}

size_t nano::ledger::get_bootstrap_weights_size () const
{
	return get_bootstrap_weights ().size ();
}

void nano::ledger::enable_pruning ()
{
	rsnano::rsn_ledger_enable_pruning (handle);
}

bool nano::ledger::pruning_enabled () const
{
	return rsnano::rsn_ledger_pruning_enabled (handle);
}

std::unordered_map<nano::account, nano::uint128_t> nano::ledger::get_bootstrap_weights () const
{
	std::unordered_map<nano::account, nano::uint128_t> weights;
	rsnano::BootstrapWeightsDto dto;
	rsnano::rsn_ledger_bootstrap_weights (handle, &dto);
	for (int i = 0; i < dto.count; ++i)
	{
		nano::account account;
		nano::uint128_t amount;
		auto & item = dto.accounts[i];
		std::copy (std::begin (item.account), std::end (item.account), std::begin (account.bytes));
		boost::multiprecision::import_bits (amount, std::begin (item.weight), std::end (item.weight), 8, true);
		weights.emplace (account, amount);
	}
	rsnano::rsn_ledger_destroy_bootstrap_weights_dto (&dto);
	return weights;
}

void nano::ledger::set_bootstrap_weights (std::unordered_map<nano::account, nano::uint128_t> const & weights_a)
{
	std::vector<rsnano::BootstrapWeightsItem> dtos;
	dtos.reserve (weights_a.size ());
	for (auto & it : weights_a)
	{
		rsnano::BootstrapWeightsItem dto;
		std::copy (std::begin (it.first.bytes), std::end (it.first.bytes), std::begin (dto.account));
		std::fill (std::begin (dto.weight), std::end (dto.weight), 0);
		boost::multiprecision::export_bits (it.second, std::rbegin (dto.weight), 8, false);
		dtos.push_back (dto);
	}
	rsnano::rsn_ledger_set_bootstrap_weights (handle, dtos.data (), dtos.size ());
}

uint64_t nano::ledger::get_bootstrap_weight_max_blocks () const
{
	return rsnano::rsn_ledger_bootstrap_weight_max_blocks (handle);
}

void nano::ledger::set_bootstrap_weight_max_blocks (uint64_t max_a)
{
	rsnano::rsn_ledger_set_bootstrap_weight_max_blocks (handle, max_a);
}

nano::uncemented_info::uncemented_info (nano::block_hash const & cemented_frontier, nano::block_hash const & frontier, nano::account const & account) :
	cemented_frontier (cemented_frontier), frontier (frontier), account (account)
{
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (ledger & ledger, std::string const & name)
{
	auto count = ledger.get_bootstrap_weights_size ();
	auto sizeof_element = sizeof (nano::account) + sizeof (nano::uint128_t);
	auto composite = std::make_unique<container_info_composite> (name);
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "bootstrap_weights", count, sizeof_element }));
	composite->add_component (collect_container_info (ledger.cache.rep_weights (), "rep_weights"));
	return composite;
}
