#include <nano/lib/blocks.hpp>
#include <nano/lib/config.hpp>
#include <nano/lib/epoch.hpp>
#include <nano/lib/numbers.hpp>
#include <nano/lib/work.hpp>
#include <nano/secure/common.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

TEST (difficulty, network_constants)
{
	auto full_thresholds = nano::work_thresholds::publish_full ();
	auto beta_thresholds = nano::work_thresholds::publish_beta ();
	auto dev_thresholds = nano::work_thresholds::publish_dev ();

	ASSERT_NEAR (8., nano::difficulty::to_multiplier (full_thresholds.get_epoch_2 (), full_thresholds.get_epoch_1 ()), 1e-10);
	ASSERT_NEAR (1 / 8., nano::difficulty::to_multiplier (full_thresholds.get_epoch_2_receive (), full_thresholds.get_epoch_1 ()), 1e-10);
	ASSERT_NEAR (1., nano::difficulty::to_multiplier (full_thresholds.get_epoch_2_receive (), full_thresholds.get_entry ()), 1e-10);
	ASSERT_NEAR (1., nano::difficulty::to_multiplier (full_thresholds.get_epoch_2 (), full_thresholds.get_base ()), 1e-10);

	ASSERT_NEAR (1 / 64., nano::difficulty::to_multiplier (beta_thresholds.get_epoch_1 (), full_thresholds.get_epoch_1 ()), 1e-10);
	ASSERT_NEAR (1., nano::difficulty::to_multiplier (beta_thresholds.get_epoch_2 (), beta_thresholds.get_epoch_1 ()), 1e-10);
	ASSERT_NEAR (1 / 2., nano::difficulty::to_multiplier (beta_thresholds.get_epoch_2_receive (), beta_thresholds.get_epoch_1 ()), 1e-10);
	ASSERT_NEAR (1., nano::difficulty::to_multiplier (beta_thresholds.get_epoch_2_receive (), beta_thresholds.get_entry ()), 1e-10);
	ASSERT_NEAR (1., nano::difficulty::to_multiplier (beta_thresholds.get_epoch_2 (), beta_thresholds.get_base ()), 1e-10);

	ASSERT_NEAR (8., nano::difficulty::to_multiplier (dev_thresholds.get_epoch_2 (), dev_thresholds.get_epoch_1 ()), 1e-10);
	ASSERT_NEAR (1 / 8., nano::difficulty::to_multiplier (dev_thresholds.get_epoch_2_receive (), dev_thresholds.get_epoch_1 ()), 1e-10);
	ASSERT_NEAR (1., nano::difficulty::to_multiplier (dev_thresholds.get_epoch_2_receive (), dev_thresholds.get_entry ()), 1e-10);
	ASSERT_NEAR (1., nano::difficulty::to_multiplier (dev_thresholds.get_epoch_2 (), dev_thresholds.get_base ()), 1e-10);

	nano::work_version version{ nano::work_version::work_1 };
	ASSERT_EQ (nano::dev::network_params.work.get_base (), nano::dev::network_params.work.get_epoch_2 ());
	ASSERT_EQ (nano::dev::network_params.work.get_base (), nano::dev::network_params.work.threshold_base (version));
	ASSERT_EQ (nano::dev::network_params.work.get_entry (), nano::dev::network_params.work.threshold_entry (version, nano::block_type::state));
	ASSERT_EQ (nano::dev::network_params.work.get_epoch_1 (), nano::dev::network_params.work.threshold_entry (version, nano::block_type::send));
	ASSERT_EQ (nano::dev::network_params.work.get_epoch_1 (), nano::dev::network_params.work.threshold_entry (version, nano::block_type::receive));
	ASSERT_EQ (nano::dev::network_params.work.get_epoch_1 (), nano::dev::network_params.work.threshold_entry (version, nano::block_type::open));
	ASSERT_EQ (nano::dev::network_params.work.get_epoch_1 (), nano::dev::network_params.work.threshold_entry (version, nano::block_type::change));
	ASSERT_EQ (nano::dev::network_params.work.get_epoch_1 (), nano::dev::network_params.work.threshold (version, nano::block_details (nano::epoch::epoch_0, false, false, false)));
	ASSERT_EQ (nano::dev::network_params.work.get_epoch_1 (), nano::dev::network_params.work.threshold (version, nano::block_details (nano::epoch::epoch_1, false, false, false)));
	ASSERT_EQ (nano::dev::network_params.work.get_epoch_1 (), nano::dev::network_params.work.threshold (version, nano::block_details (nano::epoch::epoch_1, false, false, false)));

	// Send [+ change]
	ASSERT_EQ (nano::dev::network_params.work.get_epoch_2 (), nano::dev::network_params.work.threshold (version, nano::block_details (nano::epoch::epoch_2, true, false, false)));
	// Change
	ASSERT_EQ (nano::dev::network_params.work.get_epoch_2 (), nano::dev::network_params.work.threshold (version, nano::block_details (nano::epoch::epoch_2, false, false, false)));
	// Receive [+ change] / Open
	ASSERT_EQ (nano::dev::network_params.work.get_epoch_2_receive (), nano::dev::network_params.work.threshold (version, nano::block_details (nano::epoch::epoch_2, false, true, false)));
	// Epoch
	ASSERT_EQ (nano::dev::network_params.work.get_epoch_2_receive (), nano::dev::network_params.work.threshold (version, nano::block_details (nano::epoch::epoch_2, false, false, true)));
}