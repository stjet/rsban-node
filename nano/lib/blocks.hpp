#pragma once

#include "nano/lib/config.hpp"

#include <nano/lib/block_sideband.hpp>
#include <nano/lib/epoch.hpp>
#include <nano/lib/errors.hpp>
#include <nano/lib/numbers.hpp>
#include <nano/lib/optional_ptr.hpp>
#include <nano/lib/stream.hpp>

#include <boost/property_tree/ptree_fwd.hpp>

#include <unordered_map>

namespace rsnano
{
class BlockHandle;
}

namespace nano
{
class block_visitor;
class mutable_block_visitor;
class object_stream;

class block
{
public:
	virtual ~block ();
	// Return a digest of the hashables in this block.
	nano::block_hash const & hash () const;
	// Return a digest of hashables and non-hashables in this block.
	nano::block_hash full_hash () const;
	nano::block_sideband sideband () const;
	void sideband_set (nano::block_sideband const &);
	bool has_sideband () const;
	std::string to_json () const;
	virtual uint64_t block_work () const;
	virtual void block_work_set (uint64_t);
	// Previous block in account's chain, zero for open block
	virtual nano::block_hash previous () const;
	// Previous block or account number for open blocks
	virtual nano::root root () const = 0;
	// Qualified root value based on previous() and root()
	virtual nano::qualified_root qualified_root () const;
	virtual nano::account representative () const;
	virtual void serialize (nano::stream &) const;
	virtual void serialize_json (std::string &, bool = false) const;
	virtual void serialize_json (boost::property_tree::ptree &) const;
	virtual void visit (nano::block_visitor &) const = 0;
	virtual void visit (nano::mutable_block_visitor &) = 0;
	virtual bool operator== (nano::block const &) const = 0;
	virtual nano::block_type type () const;
	virtual nano::signature block_signature () const;
	virtual void signature_set (nano::signature const &);
	virtual void sign_zero ();
	virtual bool valid_predecessor (nano::block const &) const = 0;
	// Serialized size
	static size_t size (nano::block_type);
	virtual nano::work_version work_version () const;
	// If there are any changes to the hashables, call this to update the cached hash
	void refresh ();
	bool is_send () const noexcept;
	bool is_receive () const noexcept;

public: // Direct access to the block fields or nullopt if the block type does not have the specified field
	// Returns account field or account from sideband
	nano::account account () const noexcept;
	// Account field for open/state blocks
	virtual std::optional<nano::account> account_field () const;
	// Returns the balance field or balance from sideband
	nano::amount balance () const noexcept;
	// Balance field for open/send/state blocks
	virtual std::optional<nano::amount> balance_field () const;
	// Returns the destination account for send/state blocks that are sends
	nano::account destination () const noexcept;
	// Destination account for send blocks
	virtual std::optional<nano::account> destination_field () const;
	// Link field for state blocks
	virtual std::optional<nano::link> link () const;
	// Returns the source block hash for open/receive/state blocks that are receives
	nano::block_hash source () const noexcept;
	// Source block for open/receive blocks
	virtual std::optional<nano::block_hash> source_field () const;

	rsnano::BlockHandle * get_handle () const;
	rsnano::BlockHandle * clone_handle () const;

	// gets the pointer to the block data within Rust;
	const void * get_rust_data_pointer () const;

protected:
	virtual nano::block_hash generate_hash () const;
	mutable nano::block_hash cached_hash{ 0 };
	rsnano::BlockHandle * handle;

public: // Logging
	void operator() (nano::object_stream &) const;
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
	send_block (rsnano::BlockHandle * handle_a);
	using nano::block::hash;
	nano::root root () const override;
	void visit (nano::block_visitor &) const override;
	void visit (nano::mutable_block_visitor &) override;
	bool operator== (nano::block const &) const override;
	bool operator== (nano::send_block const &) const;
	bool valid_predecessor (nano::block const &) const override;
	void zero ();
	void set_destination (nano::account account_a);
	void previous_set (nano::block_hash previous_a);
	void balance_set (nano::amount balance_a);
	static std::size_t size ();

public: // Send block fields
	std::optional<nano::amount> balance_field () const override;
	std::optional<nano::account> destination_field () const override;
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
	receive_block (rsnano::BlockHandle * handle_a);
	using nano::block::hash;
	void previous_set (nano::block_hash previous_a);
	void source_set (nano::block_hash source_a);
	nano::root root () const override;
	void visit (nano::block_visitor &) const override;
	void visit (nano::mutable_block_visitor &) override;
	bool operator== (nano::block const &) const override;
	bool operator== (nano::receive_block const &) const;
	bool valid_predecessor (nano::block const &) const override;
	void zero ();
	static std::size_t size ();

public: // Receive block fields
	std::optional<nano::block_hash> source_field () const override;
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
	open_block (rsnano::BlockHandle * handle_a);
	using nano::block::hash;
	nano::root root () const override;
	nano::account representative () const override;
	void visit (nano::block_visitor &) const override;
	void visit (nano::mutable_block_visitor &) override;
	bool operator== (nano::block const &) const override;
	bool operator== (nano::open_block const &) const;
	bool valid_predecessor (nano::block const &) const override;
	void source_set (nano::block_hash source_a);
	void account_set (nano::account account_a);
	void representative_set (nano::account account_a);
	void zero ();
	static std::size_t size ();

public: // Open block fields
	std::optional<nano::account> account_field () const override;
	std::optional<nano::block_hash> source_field () const override;
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
	change_block (rsnano::BlockHandle * handle_a);
	using nano::block::hash;
	nano::root root () const override;
	nano::account representative () const override;
	void visit (nano::block_visitor &) const override;
	void visit (nano::mutable_block_visitor &) override;
	bool operator== (nano::block const &) const override;
	bool operator== (nano::change_block const &) const;
	bool valid_predecessor (nano::block const &) const override;
	void previous_set (nano::block_hash previous_a);
	void representative_set (nano::account account_a);
	void zero ();
	static std::size_t size ();
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
	state_block (rsnano::BlockHandle * handle_a);
	using nano::block::hash;
	nano::root root () const override;
	nano::account representative () const override;
	void visit (nano::block_visitor &) const override;
	void visit (nano::mutable_block_visitor &) override;
	bool operator== (nano::block const &) const override;
	bool operator== (nano::state_block const &) const;
	nano::state_block & operator= (const nano::state_block & other);
	bool valid_predecessor (nano::block const &) const override;
	void previous_set (nano::block_hash previous_a);
	void balance_set (nano::amount balance_a);
	void account_set (nano::account account_a);
	void representative_set (nano::account account_a);
	void link_set (nano::link link);
	void zero ();
	static std::size_t size ();

public: // State block fields
	std::optional<nano::account> account_field () const override;
	std::optional<nano::amount> balance_field () const override;
	std::optional<nano::link> link () const override;
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

std::shared_ptr<nano::block> deserialize_block_json (boost::property_tree::ptree const &);
/**
 * Serialize a block prefixed with an 8-bit typecode
 */
void serialize_block (nano::stream &, nano::block const &);

void block_memory_pool_purge ();
std::shared_ptr<nano::block> block_handle_to_block (rsnano::BlockHandle * handle);
}
