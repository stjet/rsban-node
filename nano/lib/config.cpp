#include <nano/lib/blocks.hpp>
#include <nano/lib/config.hpp>

#include <boost/filesystem/path.hpp>
#include <boost/lexical_cast.hpp>

#include <valgrind/valgrind.h>

namespace
{
// useful for boost_lexical cast to allow conversion of hex strings
template <typename ElemT>
struct HexTo
{
	ElemT value;
	operator ElemT () const
	{
		return value;
	}
	friend std::istream & operator>> (std::istream & in, HexTo & out)
	{
		in >> std::hex >> out.value;
		return in;
	}
};
} // namespace

nano::work_thresholds::work_thresholds (uint64_t epoch_1_a, uint64_t epoch_2_a, uint64_t epoch_2_receive_a)
{
	rsnano::rsn_work_thresholds_create (&dto, epoch_1_a, epoch_2_a, epoch_2_receive_a);
}

nano::work_thresholds::work_thresholds (rsnano::WorkThresholdsDto dto_a) :
	dto (dto_a)
{
}

nano::work_thresholds const nano::work_thresholds::publish_full ()
{
	rsnano::WorkThresholdsDto dto;
	rsnano::rsn_work_thresholds_publish_full (&dto);
	return nano::work_thresholds (dto);
}

nano::work_thresholds const nano::work_thresholds::publish_beta ()
{
	rsnano::WorkThresholdsDto dto;
	rsnano::rsn_work_thresholds_publish_beta (&dto);
	return nano::work_thresholds (dto);
}

nano::work_thresholds const nano::work_thresholds::publish_dev ()
{
	rsnano::WorkThresholdsDto dto;
	rsnano::rsn_work_thresholds_publish_dev (&dto);
	return nano::work_thresholds (dto);
}

nano::work_thresholds const nano::work_thresholds::publish_test ()
{
	rsnano::WorkThresholdsDto dto;
	rsnano::rsn_work_thresholds_publish_test (&dto);
	return nano::work_thresholds (dto);
}

uint64_t nano::work_thresholds::get_base () const
{
	return dto.base;
}

uint64_t nano::work_thresholds::get_epoch_2 () const
{
	return dto.epoch_2;
}

uint64_t nano::work_thresholds::get_epoch_2_receive () const
{
	return dto.epoch_2_receive;
}

uint64_t nano::work_thresholds::get_entry () const
{
	return dto.entry;
}

uint64_t nano::work_thresholds::get_epoch_1 () const
{
	return dto.epoch_1;
}

uint8_t work_version_to_uint8 (nano::work_version const version_a)
{
	switch (version_a)
	{
		case nano::work_version::unspecified:
			return 0;
		case nano::work_version::work_1:
			return 1;
		default:
			return std::numeric_limits<uint8_t>::max ();
	}
}

uint64_t nano::work_thresholds::threshold_entry (nano::work_version const version_a, nano::block_type const type_a) const
{
	return rsnano::rsn_work_thresholds_threshold_entry (&dto, work_version_to_uint8 (version_a), static_cast<uint8_t> (type_a));
}

#ifndef NANO_FUZZER_TEST
uint64_t nano::work_thresholds::value (nano::root const & root_a, uint64_t work_a) const
{
	uint8_t bytes[32];
	std::copy (std::begin (root_a.bytes), std::end (root_a.bytes), std::begin (bytes));
	return rsnano::rsn_work_thresholds_value (&dto, &bytes, work_a);
}
#else
uint64_t nano::work_thresholds::value (nano::root const & root_a, uint64_t work_a) const
{
	return dto.base + 1;
}
#endif

uint64_t nano::work_thresholds::threshold (nano::block_details const & details_a) const
{
	return rsnano::rsn_work_thresholds_threshold (&dto, &details_a.dto);
}

uint64_t nano::work_thresholds::threshold (nano::work_version const version_a, nano::block_details const details_a) const
{
	return rsnano::rsn_work_thresholds_threshold2 (&dto, work_version_to_uint8 (version_a), &details_a.dto);
}

double nano::work_thresholds::normalized_multiplier (double const multiplier_a, uint64_t const threshold_a) const
{
	return rsnano::rsn_work_thresholds_normalized_multiplier (&dto, multiplier_a, threshold_a);
}

double nano::work_thresholds::denormalized_multiplier (double const multiplier_a, uint64_t const threshold_a) const
{
	return rsnano::rsn_work_thresholds_denormalized_multiplier (&dto, multiplier_a, threshold_a);
}

uint64_t nano::work_thresholds::threshold_base (nano::work_version const version_a) const
{
	return rsnano::rsn_work_thresholds_threshold_base (&dto, work_version_to_uint8 (version_a));
}

uint64_t nano::work_thresholds::difficulty (nano::work_version const version_a, nano::root const & root_a, uint64_t const work_a) const
{
	uint8_t bytes[32];
	std::copy (std::begin (root_a.bytes), std::end (root_a.bytes), std::begin (bytes));
	return rsnano::rsn_work_thresholds_difficulty (&dto, work_version_to_uint8 (version_a), &bytes, work_a);
}

uint64_t nano::work_thresholds::difficulty (nano::block const & block_a) const
{
	return difficulty (block_a.work_version (), block_a.root (), block_a.block_work ());
}

bool nano::work_thresholds::validate_entry (nano::work_version const version_a, nano::root const & root_a, uint64_t const work_a) const
{
	uint8_t bytes[32];
	std::copy (std::begin (root_a.bytes), std::end (root_a.bytes), std::begin (bytes));
	return rsnano::rsn_work_thresholds_validate_entry (&dto, work_version_to_uint8 (version_a), &bytes, work_a);
}

bool nano::work_thresholds::validate_entry (nano::block const & block_a) const
{
	return difficulty (block_a) < threshold_entry (block_a.work_version (), block_a.type ());
}

//todo: make configurable in builld script again!
static nano::networks active_network_value = nano::networks::nano_dev_network;

nano::networks nano::network_constants::active_network ()
{
	return active_network_value;
}

void nano::network_constants::set_active_network (nano::networks network_a)
{
	active_network_value = network_a;
}

bool nano::network_constants::set_active_network (std::string network_a)
{
	auto error{ false };
	if (network_a == "live")
	{
		active_network_value = nano::networks::nano_live_network;
	}
	else if (network_a == "beta")
	{
		active_network_value = nano::networks::nano_beta_network;
	}
	else if (network_a == "dev")
	{
		active_network_value = nano::networks::nano_dev_network;
	}
	else if (network_a == "test")
	{
		active_network_value = nano::networks::nano_test_network;
	}
	else
	{
		error = true;
	}
	return error;
}

namespace nano
{
char const * network_constants::active_network_err_msg = "Invalid network. Valid values are live, test, beta and dev.";

uint8_t get_major_node_version ()
{
	return boost::numeric_cast<uint8_t> (boost::lexical_cast<int> (NANO_MAJOR_VERSION_STRING));
}
uint8_t get_minor_node_version ()
{
	return boost::numeric_cast<uint8_t> (boost::lexical_cast<int> (NANO_MINOR_VERSION_STRING));
}
uint8_t get_patch_node_version ()
{
	return boost::numeric_cast<uint8_t> (boost::lexical_cast<int> (NANO_PATCH_VERSION_STRING));
}
uint8_t get_pre_release_node_version ()
{
	return boost::numeric_cast<uint8_t> (boost::lexical_cast<int> (NANO_PRE_RELEASE_VERSION_STRING));
}

std::string get_env_or_default (char const * variable_name, std::string default_value)
{
	auto value = getenv (variable_name);
	return value ? value : default_value;
}

uint64_t get_env_threshold_or_default (char const * variable_name, uint64_t const default_value)
{
	auto * value = getenv (variable_name);
	return value ? boost::lexical_cast<HexTo<uint64_t>> (value) : default_value;
}

uint16_t test_node_port ()
{
	auto test_env = nano::get_env_or_default ("NANO_TEST_NODE_PORT", "17075");
	return boost::lexical_cast<uint16_t> (test_env);
}
uint16_t test_rpc_port ()
{
	auto test_env = nano::get_env_or_default ("NANO_TEST_RPC_PORT", "17076");
	return boost::lexical_cast<uint16_t> (test_env);
}
uint16_t test_ipc_port ()
{
	auto test_env = nano::get_env_or_default ("NANO_TEST_IPC_PORT", "17077");
	return boost::lexical_cast<uint16_t> (test_env);
}
uint16_t test_websocket_port ()
{
	auto test_env = nano::get_env_or_default ("NANO_TEST_WEBSOCKET_PORT", "17078");
	return boost::lexical_cast<uint16_t> (test_env);
}

std::array<uint8_t, 2> test_magic_number ()
{
	auto test_env = get_env_or_default ("NANO_TEST_MAGIC_NUMBER", "RX");
	std::array<uint8_t, 2> ret;
	std::copy (test_env.begin (), test_env.end (), ret.data ());
	return ret;
}

void force_nano_dev_network ()
{
	nano::network_constants::set_active_network (nano::networks::nano_dev_network);
}

bool running_within_valgrind ()
{
	return (RUNNING_ON_VALGRIND > 0);
}

std::string get_config_path (boost::filesystem::path const & data_path)
{
	return (data_path / "config.json").string ();
}

std::string get_rpc_config_path (boost::filesystem::path const & data_path)
{
	return (data_path / "rpc_config.json").string ();
}

std::string get_node_toml_config_path (boost::filesystem::path const & data_path)
{
	return (data_path / "config-node.toml").string ();
}

std::string get_rpc_toml_config_path (boost::filesystem::path const & data_path)
{
	return (data_path / "config-rpc.toml").string ();
}

std::string get_qtwallet_toml_config_path (boost::filesystem::path const & data_path)
{
	return (data_path / "config-qtwallet.toml").string ();
}

std::string get_access_toml_config_path (boost::filesystem::path const & data_path)
{
	return (data_path / "config-access.toml").string ();
}

std::string get_tls_toml_config_path (boost::filesystem::path const & data_path)
{
	return (data_path / "config-tls.toml").string ();
}
} // namespace nano
