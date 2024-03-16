#pragma once

#include <nano/lib/block_type.hpp>
#include <nano/lib/epoch.hpp>
#include <nano/lib/numbers.hpp>
#include <nano/lib/rsnano.hpp>
#include <nano/lib/stream.hpp>
#include <nano/lib/timer.hpp>

#include <cstdint>
#include <memory>

namespace nano
{
class object_stream;
}

namespace nano
{
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
	block_sideband (rsnano::BlockSidebandDto const & dto);
	block_sideband (nano::account const &, nano::block_hash const &, nano::amount const &, uint64_t const, nano::seconds_t const local_timestamp, nano::block_details const &, nano::epoch const source_epoch_a);
	block_sideband (nano::account const &, nano::block_hash const &, nano::amount const &, uint64_t const, nano::seconds_t const local_timestamp, nano::epoch const epoch_a, bool const is_send, bool const is_receive, bool const is_epoch, nano::epoch const source_epoch_a);

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
	rsnano::BlockSidebandDto const & as_dto () const;

private:
	rsnano::BlockSidebandDto dto;
};
} // namespace nano
