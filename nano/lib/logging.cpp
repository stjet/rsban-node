#include "nano/lib/rsnano.hpp"

#include <nano/lib/config.hpp>
#include <nano/lib/logging.hpp>
#include <nano/lib/utility.hpp>

#include <fmt/chrono.h>
#include <spdlog/pattern_formatter.h>
#include <spdlog/sinks/basic_file_sink.h>
#include <spdlog/sinks/rotating_file_sink.h>
#include <spdlog/sinks/stdout_color_sinks.h>
#include <spdlog/sinks/stdout_sinks.h>

nano::logger & nano::default_logger ()
{
	static nano::logger logger{ "default" };
	return logger;
}

/*
 * logger
 */

bool nano::logger::global_initialized{ false };
nano::log::level nano::logger::min_level{ nano::log::level::info };

void nano::logger::initialize ()
{
	rsnano::rsn_log_init ();
	min_level = static_cast<nano::log::level> (rsnano::rsn_log_min_level ());
	global_initialized = true;
}

void nano::logger::initialize_for_tests ()
{
	rsnano::rsn_log_init_test ();
	min_level = static_cast<nano::log::level> (rsnano::rsn_log_min_level ());
	global_initialized = true;
}

/*
 * logger
 */

nano::logger::logger (std::string identifier)
{
	release_assert (global_initialized, "logging should be initialized before creating a logger");
}

nano::logger::~logger ()
{
}

void nano::log_with_rust (nano::log::level level, nano::log::type type, const char * message, std::size_t size)
{
	rsnano::rsn_log (static_cast<uint8_t> (level), message, size);
}
