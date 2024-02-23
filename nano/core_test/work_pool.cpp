#include <nano/crypto_lib/random_pool.hpp>
#include <nano/lib/logging.hpp>
#include <nano/lib/timer.hpp>
#include <nano/lib/work.hpp>
#include <nano/node/openclconfig.hpp>
#include <nano/node/openclwork.hpp>
#include <nano/secure/common.hpp>
#include <nano/secure/utility.hpp>

#include <gtest/gtest.h>

#include <future>

TEST (work, opencl)
{
	nano::logger logger;
	bool error = false;
	nano::opencl_environment environment (error);
	ASSERT_TRUE (!error || !nano::opencl_loaded);

	if (environment.platforms.empty () || environment.platforms.begin ()->devices.empty ())
	{
		GTEST_SKIP () << "Device with OpenCL support not found. Skipping OpenCL test" << std::endl;
	}

	nano::opencl_config config (0, 0, 16 * 1024);
	auto opencl (nano::opencl_work::create (true, config, logger, nano::dev::network_params.work));
	ASSERT_TRUE (opencl);

	// 0 threads, should add 1 for managing OpenCL
	bool opencl_function_called = false;
	nano::work_pool pool{ nano::dev::network_params.network, 0, std::chrono::nanoseconds (0), 
		[&opencl, &opencl_function_called] (nano::work_version const version_a, nano::root const & root_a, uint64_t difficulty_a, nano::work_ticket ticket_a) {
			opencl_function_called = true;
			return opencl->generate_work (version_a, root_a, difficulty_a);
		} };

	ASSERT_TRUE (pool.has_opencl ());

	nano::root root;
	uint64_t difficulty (0xff00000000000000);
	uint64_t difficulty_add (0x000f000000000000);
	for (auto i (0); i < 16; ++i)
	{
		nano::random_pool::generate_block (root.bytes.data (), root.bytes.size ());
		auto nonce_opt = pool.generate (nano::work_version::work_1, root, difficulty);
		ASSERT_TRUE (nonce_opt.has_value ());
		auto nonce = nonce_opt.get ();
		ASSERT_GE (nano::dev::network_params.work.difficulty (nano::work_version::work_1, root, nonce), difficulty);
		difficulty += difficulty_add;
	}

	ASSERT_TRUE (opencl_function_called);
}
