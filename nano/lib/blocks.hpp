#pragma once

#include <nano/crypto/blake2/blake2.h>
#include <nano/lib/epoch.hpp>
#include <nano/lib/errors.hpp>
#include <nano/lib/numbers.hpp>
#include <nano/lib/optional_ptr.hpp>
#include <nano/lib/rsnano.hpp>
#include <nano/lib/stream.hpp>
#include <nano/lib/utility.hpp>
#include <nano/lib/work.hpp>

#include <boost/property_tree/ptree_fwd.hpp>

#include <unordered_map>

namespace nano
{
class block_visitor;
class mutable_block_visitor;
enum class block_type : uint8_t
{
	invalid = 0,
	not_a_block = 1,
	send = 2,
	receive = 3,
	open = 4,
	change = 5,
	state = 6
};
class block_details
{
	static_assert (std::is_same<std::underlying_type<nano::epoch>::type, uint8_t> (), "Epoch enum is not the proper type");
	static_assert (static_cast<uint8_t> (nano::epoch::max) < (1 << 5), "Epoch max is too large for the sideband");

public:
	block_details ();
	block_details (nano::epoch const epoch_a, bool const is_send_a, bool const is_receive_a, bool const is_epoch_a);
	block_details (rsnano::BlockDetailsDto dto_a);
	constexpr static size_t size ()
	{
		return 1;
	}
	bool operator== (block_details const & other_a) const;
	void serialize (nano::stream &) const;
	bool deserialize (nano::stream &);
	nano::epoch epoch () const;
	bool is_send () const;
	bool is_receive () const;
	bool is_epoch () const;

	rsnano::BlockDetailsDto dto;
};

std::string state_subtype (nano::block_details const);

class block_sideband final
{
public:
	block_sideband ();
	block_sideband (nano::account const &, nano::block_hash const &, nano::amount const &, uint64_t const, uint64_t const, nano::block_details const &, nano::epoch const source_epoch_a);
	block_sideband (nano::account const &, nano::block_hash const &, nano::amount const &, uint64_t const, uint64_t const, nano::epoch const epoch_a, bool const is_send, bool const is_receive, bool const is_epoch, nano::epoch const source_epoch_a);

	void serialize (nano::stream &, nano::block_type) const;
	bool deserialize (nano::stream &, nano::block_type);

	nano::epoch source_epoch () const;
	void set_source_epoch (nano::epoch epoch);
	uint64_t height () const;
	void set_height (uint64_t h);
	uint64_t timestamp () const;
	void set_timestamp (uint64_t ts);
	nano::block_details details () const;
	nano::block_hash successor () const;
	void set_successor (nano::block_hash successor_a);
	nano::account account () const;
	nano::amount balance () const;

	static size_t size (nano::block_type);

private:
	rsnano::BlockSidebandDto dto;
};

class block
{
public:
	// Return a digest of the hashables in this block.
	nano::block_hash const & hash () const;
	// Return a digest of hashables and non-hashables in this block.
	nano::block_hash full_hash () const;
	nano::block_sideband const & sideband () const;
	void sideband_set (nano::block_sideband const &);
	bool has_sideband () const;
	std::string to_json () const;
	virtual uint64_t block_work () const = 0;
	virtual void block_work_set (uint64_t) = 0;
	virtual nano::account account () const;
	// Previous block in account's chain, zero for open block
	virtual nano::block_hash previous () const = 0;
	// Source block for open/receive blocks, zero otherwise.
	virtual nano::block_hash source () const;
	// Destination account for send blocks, zero otherwise.
	virtual nano::account destination () const;
	// Previous block or account number for open blocks
	virtual nano::root root () const = 0;
	// Qualified root value based on previous() and root()
	virtual nano::qualified_root qualified_root () const;
	// Link field for state blocks, zero otherwise.
	virtual nano::link link () const;
	virtual nano::account representative () const;
	virtual nano::amount balance () const;
	virtual void serialize (nano::stream &) const = 0;
	virtual void serialize_json (std::string &, bool = false) const = 0;
	virtual void serialize_json (boost::property_tree::ptree &) const = 0;
	virtual void visit (nano::block_visitor &) const = 0;
	virtual void visit (nano::mutable_block_visitor &) = 0;
	virtual bool operator== (nano::block const &) const = 0;
	virtual nano::block_type type () const = 0;
	virtual nano::signature block_signature () const = 0;
	virtual void signature_set (nano::signature const &) = 0;
	virtual ~block () = default;
	virtual bool valid_predecessor (nano::block const &) const = 0;
	// Serialized size
	static size_t size (nano::block_type);
	virtual nano::work_version work_version () const;
	// If there are any changes to the hashables, call this to update the cached hash
	void refresh ();

protected:
	mutable nano::block_hash cached_hash{ 0 };
	/**
	 * Contextual details about a block, some fields may or may not be set depending on block type.
	 * This field is set via sideband_set in ledger processing or deserializing blocks from the database.
	 * Otherwise it may be null (for example, an old block or fork).
	 */
	nano::optional_ptr<nano::block_sideband> sideband_m;

	virtual nano::block_hash generate_hash () const = 0;
};

class send_block final : public nano::block
{
public:
	send_block ();
	send_block (nano::block_hash const &, nano::account const &, nano::amount const &, nano::raw_key const &, nano::public_key const &, uint64_t);
	send_block (bool &, nano::stream &);
	send_block (bool &, boost::property_tree::ptree const &);
	send_block (const send_block &);
	send_block (send_block && other);
	send_block (rsnano::SendBlockHandle * handle_a);
	virtual ~send_block ();
	using nano::block::hash;
	uint64_t block_work () const override;
	void block_work_set (uint64_t) override;
	nano::block_hash previous () const override;
	nano::account destination () const override;
	nano::root root () const override;
	nano::amount balance () const override;
	void serialize (nano::stream &) const override;
	void serialize_json (std::string &, bool = false) const override;
	void serialize_json (boost::property_tree::ptree &) const override;
	void visit (nano::block_visitor &) const override;
	void visit (nano::mutable_block_visitor &) override;
	nano::block_type type () const override;
	nano::signature block_signature () const override;
	void signature_set (nano::signature const &) override;
	bool operator== (nano::block const &) const override;
	bool operator== (nano::send_block const &) const;
	bool valid_predecessor (nano::block const &) const override;
	void zero ();
	void set_destination (nano::account account_a);
	void previous_set (nano::block_hash previous_a);
	void balance_set (nano::amount balance_a);
	void sign_zero ();
	static std::size_t size ();

protected:
	nano::block_hash generate_hash () const override;

private:
	rsnano::SendBlockHandle * handle;
};

class receive_block : public nano::block
{
public:
	receive_block ();
	receive_block (nano::block_hash const &, nano::block_hash const &, nano::raw_key const &, nano::public_key const &, uint64_t);
	receive_block (bool &, nano::stream &);
	receive_block (bool &, boost::property_tree::ptree const &);
	receive_block (const nano::receive_block &);
	receive_block (nano::receive_block &&);
	receive_block (rsnano::ReceiveBlockHandle * handle_a);
	virtual ~receive_block ();
	using nano::block::hash;
	uint64_t block_work () const override;
	void block_work_set (uint64_t) override;
	nano::block_hash previous () const override;
	void previous_set (nano::block_hash previous_a);
	nano::block_hash source () const override;
	void source_set (nano::block_hash source_a);
	nano::root root () const override;
	void serialize (nano::stream &) const override;
	void serialize_json (std::string &, bool = false) const override;
	void serialize_json (boost::property_tree::ptree &) const override;
	void visit (nano::block_visitor &) const override;
	void visit (nano::mutable_block_visitor &) override;
	nano::block_type type () const override;
	nano::signature block_signature () const override;
	void signature_set (nano::signature const &) override;
	bool operator== (nano::block const &) const override;
	bool operator== (nano::receive_block const &) const;
	bool valid_predecessor (nano::block const &) const override;
	void sign_zero ();
	void zero ();
	static std::size_t size ();

protected:
	nano::block_hash generate_hash () const override;

private:
	rsnano::ReceiveBlockHandle * handle;
};
class open_block : public nano::block
{
public:
	open_block ();
	open_block (nano::block_hash const &, nano::account const &, nano::account const &, nano::raw_key const &, nano::public_key const &, uint64_t);
	open_block (nano::block_hash const &, nano::account const &, nano::account const &, std::nullptr_t);
	open_block (bool &, nano::stream &);
	open_block (bool &, boost::property_tree::ptree const &);
	open_block (const nano::open_block &);
	open_block (nano::open_block &&);
	open_block (rsnano::OpenBlockHandle * handle_a);
	virtual ~open_block ();
	using nano::block::hash;
	uint64_t block_work () const override;
	void block_work_set (uint64_t) override;
	nano::block_hash previous () const override;
	nano::account account () const override;
	nano::block_hash source () const override;
	nano::root root () const override;
	nano::account representative () const override;
	void serialize (nano::stream &) const override;
	void serialize_json (std::string &, bool = false) const override;
	void serialize_json (boost::property_tree::ptree &) const override;
	void visit (nano::block_visitor &) const override;
	void visit (nano::mutable_block_visitor &) override;
	nano::block_type type () const override;
	nano::signature block_signature () const override;
	void signature_set (nano::signature const &) override;
	bool operator== (nano::block const &) const override;
	bool operator== (nano::open_block const &) const;
	bool valid_predecessor (nano::block const &) const override;
	void sign_zero ();
	void source_set (nano::block_hash source_a);
	void account_set (nano::account account_a);
	void representative_set (nano::account account_a);
	void zero ();
	static std::size_t size ();

protected:
	nano::block_hash generate_hash () const override;

private:
	rsnano::OpenBlockHandle * handle;
};
class change_block : public nano::block
{
public:
	change_block ();
	change_block (nano::block_hash const &, nano::account const &, nano::raw_key const &, nano::public_key const &, uint64_t);
	change_block (bool &, nano::stream &);
	change_block (bool &, boost::property_tree::ptree const &);
	change_block (const nano::change_block &);
	change_block (nano::change_block &&);
	change_block (rsnano::ChangeBlockHandle * handle_a);
	virtual ~change_block ();
	using nano::block::hash;
	uint64_t block_work () const override;
	void block_work_set (uint64_t) override;
	nano::block_hash previous () const override;
	nano::root root () const override;
	nano::account representative () const override;
	void serialize (nano::stream &) const override;
	void serialize_json (std::string &, bool = false) const override;
	void serialize_json (boost::property_tree::ptree &) const override;
	void visit (nano::block_visitor &) const override;
	void visit (nano::mutable_block_visitor &) override;
	nano::block_type type () const override;
	nano::signature block_signature () const override;
	void signature_set (nano::signature const &) override;
	bool operator== (nano::block const &) const override;
	bool operator== (nano::change_block const &) const;
	bool valid_predecessor (nano::block const &) const override;
	void previous_set (nano::block_hash previous_a);
	void representative_set (nano::account account_a);
	void sign_zero ();
	void zero ();
	static std::size_t size ();

protected:
	nano::block_hash generate_hash () const override;

private:
	rsnano::ChangeBlockHandle * handle;
};
class state_block : public nano::block
{
public:
	state_block ();
	state_block (nano::account const &, nano::block_hash const &, nano::account const &, nano::amount const &, nano::link const &, nano::raw_key const &, nano::public_key const &, uint64_t);
	state_block (bool &, nano::stream &);
	state_block (bool &, boost::property_tree::ptree const &);
	state_block (const nano::state_block &);
	state_block (nano::state_block &&);
	state_block (rsnano::StateBlockHandle * handle_a);
	virtual ~state_block ();
	using nano::block::hash;
	uint64_t block_work () const override;
	void block_work_set (uint64_t) override;
	nano::block_hash previous () const override;
	nano::account account () const override;
	nano::root root () const override;
	nano::link link () const override;
	nano::account representative () const override;
	nano::amount balance () const override;
	void serialize (nano::stream &) const override;
	void serialize_json (std::string &, bool = false) const override;
	void serialize_json (boost::property_tree::ptree &) const override;
	void visit (nano::block_visitor &) const override;
	void visit (nano::mutable_block_visitor &) override;
	nano::block_type type () const override;
	nano::signature block_signature () const override;
	void signature_set (nano::signature const &) override;
	bool operator== (nano::block const &) const override;
	bool operator== (nano::state_block const &) const;
	nano::state_block & operator= (const nano::state_block & other);
	bool valid_predecessor (nano::block const &) const override;
	void previous_set (nano::block_hash previous_a);
	void balance_set (nano::amount balance_a);
	void account_set (nano::account account_a);
	void representative_set (nano::account account_a);
	void link_set (nano::link link);
	void sign_zero ();
	void zero ();
	static std::size_t size ();

protected:
	nano::block_hash generate_hash () const override;

private:
	rsnano::StateBlockHandle * handle;
};
class block_visitor
{
public:
	virtual void send_block (nano::send_block const &) = 0;
	virtual void receive_block (nano::receive_block const &) = 0;
	virtual void open_block (nano::open_block const &) = 0;
	virtual void change_block (nano::change_block const &) = 0;
	virtual void state_block (nano::state_block const &) = 0;
	virtual ~block_visitor () = default;
};
class mutable_block_visitor
{
public:
	virtual void send_block (nano::send_block &) = 0;
	virtual void receive_block (nano::receive_block &) = 0;
	virtual void open_block (nano::open_block &) = 0;
	virtual void change_block (nano::change_block &) = 0;
	virtual void state_block (nano::state_block &) = 0;
	virtual ~mutable_block_visitor () = default;
};
/**
 * This class serves to find and return unique variants of a block in order to minimize memory usage
 */
class block_uniquer
{
public:
	using value_type = std::pair<nano::uint256_union const, std::weak_ptr<nano::block>>;

	std::shared_ptr<nano::block> unique (std::shared_ptr<nano::block> const &);
	size_t size ();

private:
	nano::mutex mutex{ mutex_identifier (mutexes::block_uniquer) };
	std::unordered_map<std::remove_const_t<value_type::first_type>, value_type::second_type> blocks;
	static unsigned constexpr cleanup_count = 2;
};

std::unique_ptr<container_info_component> collect_container_info (block_uniquer & block_uniquer, std::string const & name);

std::shared_ptr<nano::block> deserialize_block (nano::stream &);
std::shared_ptr<nano::block> deserialize_block (nano::stream &, nano::block_type, nano::block_uniquer * = nullptr);
std::shared_ptr<nano::block> deserialize_block_json (boost::property_tree::ptree const &, nano::block_uniquer * = nullptr);
void serialize_block (nano::stream &, nano::block const &);
void block_memory_pool_purge ();
}
