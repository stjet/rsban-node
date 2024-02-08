#pragma once

#include <nano/lib/logging_enums.hpp>
#include <nano/lib/tomlconfig.hpp>

#include <initializer_list>
#include <memory>
#include <shared_mutex>
#include <sstream>

#include <spdlog/spdlog.h>

namespace nano
{
class log_config final
{
public:
	nano::error serialize_toml (nano::tomlconfig &) const;
	nano::error deserialize_toml (nano::tomlconfig &);

private:
	void serialize (nano::tomlconfig &) const;
	void deserialize (nano::tomlconfig &);

public:
	nano::log::level default_level{ nano::log::level::info };
	nano::log::level flush_level{ nano::log::level::error };

	using logger_id_t = std::pair<nano::log::type, nano::log::detail>;
	std::map<logger_id_t, nano::log::level> levels;

	struct console_config
	{
		bool enable{ true };
		bool colors{ true };
		bool to_cerr{ false };
	};

	struct file_config
	{
		bool enable{ true };
		std::size_t max_size{ 32 * 1024 * 1024 };
		std::size_t rotation_count{ 4 };
	};

	console_config console;
	file_config file;

public: // Predefined defaults
	static logger_id_t parse_logger_id (std::string const &);

private:
	/// Returns placeholder log levels for all loggers
	static std::map<logger_id_t, nano::log::level> default_levels (nano::log::level);
};

nano::log_config load_log_config (nano::log_config fallback, std::filesystem::path const & data_path, std::vector<std::string> const & config_overrides = {});

void log_with_rust (nano::log::level level, nano::log::type tag, const char * message, std::size_t size);

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
	void log (nano::log::level level, nano::log::type tag, std::string const & message)
	{
		if (level >= min_level)
		{
			nano::log_with_rust (level, tag, message.c_str (), message.length ());
		}
	}

	template <class... Args>
	void log (nano::log::level level, nano::log::type tag, spdlog::format_string_t<Args...> fmt, Args &&... args)
	{
		if (level >= min_level)
		{
			spdlog::memory_buf_t buf;
			fmt::vformat_to (fmt::appender (buf), fmt, fmt::make_format_args (args...));
			nano::log_with_rust (level, tag, buf.data (), buf.size ());
		}
	}

	template <class... Args>
	void debug (nano::log::type tag, spdlog::format_string_t<Args...> fmt, Args &&... args)
	{
		if (nano::log::level::debug >= min_level)
		{
			spdlog::memory_buf_t buf;
			fmt::vformat_to (fmt::appender (buf), fmt, fmt::make_format_args (args...));
			nano::log_with_rust (nano::log::level::debug, tag, buf.data (), buf.size ());
		}
	}

	template <class... Args>
	void info (nano::log::type tag, spdlog::format_string_t<Args...> fmt, Args &&... args)
	{
		if (nano::log::level::info >= min_level)
		{
			spdlog::memory_buf_t buf;
			fmt::vformat_to (fmt::appender (buf), fmt, fmt::make_format_args (args...));
			nano::log_with_rust (nano::log::level::info, tag, buf.data (), buf.size ());
		}
	}

	void info (nano::log::type tag, std::string const & message)
	{
		if (nano::log::level::info >= min_level)
		{
			nano::log_with_rust (nano::log::level::info, tag, message.c_str (), message.length ());
		}
	}

	template <class... Args>
	void warn (nano::log::type tag, spdlog::format_string_t<Args...> fmt, Args &&... args)
	{
		if (nano::log::level::warn >= min_level)
		{
			spdlog::memory_buf_t buf;
			fmt::vformat_to (fmt::appender (buf), fmt, fmt::make_format_args (args...));
			nano::log_with_rust (nano::log::level::warn, tag, buf.data (), buf.size ());
		}
	}

	template <class... Args>
	void error (nano::log::type tag, spdlog::format_string_t<Args...> fmt, Args &&... args)
	{
		if (nano::log::level::error >= min_level)
		{
			spdlog::memory_buf_t buf;
			fmt::vformat_to (fmt::appender (buf), fmt, fmt::make_format_args (args...));
			nano::log_with_rust (nano::log::level::error, tag, buf.data (), buf.size ());
		}
	}

	template <class... Args>
	void critical (nano::log::type tag, spdlog::format_string_t<Args...> fmt, Args &&... args)
	{
		if (nano::log::level::critical >= min_level)
		{
			spdlog::memory_buf_t buf;
			fmt::vformat_to (fmt::appender (buf), fmt, fmt::make_format_args (args...));
			nano::log_with_rust (nano::log::level::critical, tag, buf.data (), buf.size ());
		}
	}
};

/**
 * Returns a logger instance that can be used before node specific logging is available.
 * Should only be used for logging that happens during startup and initialization, since it won't contain node specific identifier.
 */
nano::logger & default_logger ();
}
