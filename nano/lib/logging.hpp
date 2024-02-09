#pragma once

#include <nano/lib/logging_enums.hpp>
#include <initializer_list>
#include <memory>
#include <shared_mutex>
#include <sstream>
#include <spdlog/spdlog.h>

namespace nano
{
void log_with_rust (nano::log::level level, nano::log::type type, const char * message, std::size_t size);

class logger final
{
public:
	explicit logger (std::string identifier = "");
	~logger ();

	// Disallow copies
	logger (logger const &) = delete;

public:
	static void initialize ();
	static void initialize_for_tests ();

private:
	static bool global_initialized;
	static nano::log::level min_level;

public:
	void log (nano::log::level level, nano::log::type type, std::string const & message)
	{
		if (level >= min_level)
		{
			nano::log_with_rust (level, type, message.c_str (), message.length ());
		}
	}

	template <class... Args>
	void log (nano::log::level level, nano::log::type type, spdlog::format_string_t<Args...> fmt, Args &&... args)
	{
		if (level >= min_level)
		{
			spdlog::memory_buf_t buf;
			fmt::vformat_to (fmt::appender (buf), fmt, fmt::make_format_args (args...));
			nano::log_with_rust (level, type, buf.data (), buf.size ());
		}
	}

	template <class... Args>
	void debug (nano::log::type type, spdlog::format_string_t<Args...> fmt, Args &&... args)
	{
		if (nano::log::level::debug >= min_level)
		{
			spdlog::memory_buf_t buf;
			fmt::vformat_to (fmt::appender (buf), fmt, fmt::make_format_args (args...));
			nano::log_with_rust (nano::log::level::debug, type, buf.data (), buf.size ());
		}
	}

	template <class... Args>
	void info (nano::log::type type, spdlog::format_string_t<Args...> fmt, Args &&... args)
	{
		if (nano::log::level::info >= min_level)
		{
			spdlog::memory_buf_t buf;
			fmt::vformat_to (fmt::appender (buf), fmt, fmt::make_format_args (args...));
			nano::log_with_rust (nano::log::level::info, type, buf.data (), buf.size ());
		}
	}

	void info (nano::log::type type, std::string const & message)
	{
		if (nano::log::level::info >= min_level)
		{
			nano::log_with_rust (nano::log::level::info, type, message.c_str (), message.length ());
		}
	}

	template <class... Args>
	void warn (nano::log::type type, spdlog::format_string_t<Args...> fmt, Args &&... args)
	{
		if (nano::log::level::warn >= min_level)
		{
			spdlog::memory_buf_t buf;
			fmt::vformat_to (fmt::appender (buf), fmt, fmt::make_format_args (args...));
			nano::log_with_rust (nano::log::level::warn, type, buf.data (), buf.size ());
		}
	}

	template <class... Args>
	void error (nano::log::type type, spdlog::format_string_t<Args...> fmt, Args &&... args)
	{
		if (nano::log::level::error >= min_level)
		{
			spdlog::memory_buf_t buf;
			fmt::vformat_to (fmt::appender (buf), fmt, fmt::make_format_args (args...));
			nano::log_with_rust (nano::log::level::error, type, buf.data (), buf.size ());
		}
	}

	template <class... Args>
	void critical (nano::log::type type, spdlog::format_string_t<Args...> fmt, Args &&... args)
	{
		if (nano::log::level::critical >= min_level)
		{
			spdlog::memory_buf_t buf;
			fmt::vformat_to (fmt::appender (buf), fmt, fmt::make_format_args (args...));
			nano::log_with_rust (nano::log::level::critical, type, buf.data (), buf.size ());
		}
	}
};

/**
 * Returns a logger instance that can be used before node specific logging is available.
 * Should only be used for logging that happens during startup and initialization, since it won't contain node specific identifier.
 */
nano::logger & default_logger ();
}
