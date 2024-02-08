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
	rsnano::rsn_log_init();
	min_level = static_cast<nano::log::level> (rsnano::rsn_log_min_level ());
	global_initialized = true;
}

// Custom log formatter flags
namespace
{
/// Takes a qualified identifier in the form `node_identifier::tag` and splits it into a pair of `identifier` and `tag`
/// It is a limitation of spldlog that we cannot attach additional data to the logger, so we have to encode the node identifier in the logger name
/// @returns <node identifier, tag>

std::pair<std::string_view, std::string_view> split_qualified_identifier (std::string_view qualified_identifier)
{
	auto pos = qualified_identifier.find ("::");
	debug_assert (pos != std::string_view::npos); // This should never happen, since the default logger name formatter always adds the tag
	if (pos == std::string_view::npos)
	{
		return { std::string_view{}, qualified_identifier };
	}
	else
	{
		return { qualified_identifier.substr (0, pos), qualified_identifier.substr (pos + 2) };
	}
}

class identifier_formatter_flag : public spdlog::custom_flag_formatter
{
public:
	void format (const spdlog::details::log_msg & msg, const std::tm & tm, spdlog::memory_buf_t & dest) override
	{
		// Extract identifier and tag from logger name
		auto [identifier, tag] = split_qualified_identifier (std::string_view (msg.logger_name.data (), msg.logger_name.size ()));
		dest.append (identifier.data (), identifier.data () + identifier.size ());
	}

	std::unique_ptr<custom_flag_formatter> clone () const override
	{
		return spdlog::details::make_unique<identifier_formatter_flag> ();
	}
};

class tag_formatter_flag : public spdlog::custom_flag_formatter
{
public:
	void format (const spdlog::details::log_msg & msg, const std::tm & tm, spdlog::memory_buf_t & dest) override
	{
		// Extract identifier and tag from logger name
		auto [identifier, tag] = split_qualified_identifier (std::string_view (msg.logger_name.data (), msg.logger_name.size ()));
		dest.append (tag.data (), tag.data () + tag.size ());
	}

	std::unique_ptr<custom_flag_formatter> clone () const override
	{
		return spdlog::details::make_unique<tag_formatter_flag> ();
	}
};
}

void nano::logger::initialize_for_tests ()
{
	rsnano::rsn_log_init_test();
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

/*
 * logging config
 */

nano::error nano::log_config::serialize_toml (nano::tomlconfig & toml) const
{
	nano::tomlconfig config_toml;
	serialize (config_toml);
	toml.put_child ("log", config_toml);

	return toml.get_error ();
}

nano::error nano::log_config::deserialize_toml (nano::tomlconfig & toml)
{
	try
	{
		auto logging_l = toml.get_optional_child ("log");
		if (logging_l)
		{
			deserialize (*logging_l);
		}
	}
	catch (std::invalid_argument const & ex)
	{
		toml.get_error ().set (ex.what ());
	}

	return toml.get_error ();
}

void nano::log_config::serialize (nano::tomlconfig & toml) const
{
	toml.put ("default_level", std::string{ to_string (default_level) });

	nano::tomlconfig console_config;
	console_config.put ("enable", console.enable);
	console_config.put ("to_cerr", console.to_cerr);
	console_config.put ("colors", console.colors);
	toml.put_child ("console", console_config);

	nano::tomlconfig file_config;
	file_config.put ("enable", file.enable);
	file_config.put ("max_size", file.max_size);
	file_config.put ("rotation_count", file.rotation_count);
	toml.put_child ("file", file_config);

	nano::tomlconfig levels_config;
	for (auto const & [logger_id, level] : levels)
	{
		auto logger_name = to_string (logger_id.first);
		levels_config.put (std::string{ logger_name }, std::string{ to_string (level) });
	}
	toml.put_child ("levels", levels_config);
}

void nano::log_config::deserialize (nano::tomlconfig & toml)
{
	if (toml.has_key ("default_level"))
	{
		auto default_level_l = toml.get<std::string> ("default_level");
		default_level = nano::log::parse_level (default_level_l);
	}

	if (toml.has_key ("console"))
	{
		auto console_config = toml.get_required_child ("console");
		console_config.get ("enable", console.enable);
		console_config.get ("to_cerr", console.to_cerr);
		console_config.get ("colors", console.colors);
	}

	if (toml.has_key ("file"))
	{
		auto file_config = toml.get_required_child ("file");
		file_config.get ("enable", file.enable);
		file_config.get ("max_size", file.max_size);
		file_config.get ("rotation_count", file.rotation_count);
	}

	if (toml.has_key ("levels"))
	{
		auto levels_config = toml.get_required_child ("levels");
		for (auto & level : levels_config.get_values<std::string> ())
		{
			try
			{
				auto & [name_str, level_str] = level;
				auto logger_level = nano::log::parse_level (level_str);
				auto logger_id = parse_logger_id (name_str);

				levels[logger_id] = logger_level;
			}
			catch (std::invalid_argument const & ex)
			{
				// Ignore but warn about invalid logger names
				std::cerr << "Problem processing log config: " << ex.what () << std::endl;
			}
		}
	}
}

/**
 * Parse `logger_name[:logger_detail]` into a pair of `log::type` and `log::detail`
 * @throw std::invalid_argument if `logger_name` or `logger_detail` are invalid
 */
nano::log_config::logger_id_t nano::log_config::parse_logger_id (const std::string & logger_name)
{
	auto pos = logger_name.find ("::");
	if (pos == std::string::npos)
	{
		return { nano::log::parse_type (logger_name), nano::log::detail::all };
	}
	else
	{
		auto logger_type = logger_name.substr (0, pos);
		auto logger_detail = logger_name.substr (pos + 1);

		return { nano::log::parse_type (logger_type), nano::log::parse_detail (logger_detail) };
	}
}

std::map<nano::log_config::logger_id_t, nano::log::level> nano::log_config::default_levels (nano::log::level default_level)
{
	std::map<nano::log_config::logger_id_t, nano::log::level> result;
	for (auto const & type : nano::log::all_types ())
	{
		result.emplace (std::make_pair (type, nano::log::detail::all), default_level);
	}
	return result;
}

/*
 * config loading
 */

// Using std::cerr here, since logging may not be initialized yet
nano::log_config nano::load_log_config (nano::log_config fallback, const std::filesystem::path & data_path, const std::vector<std::string> & config_overrides)
{
	const std::string config_filename = "config-log.toml";
	try
	{
		auto config = nano::load_config_file<nano::log_config> (fallback, config_filename, data_path, config_overrides);

		// Parse default log level from environment variable, e.g. "NANO_LOG=debug"
		auto env_level = nano::get_env ("NANO_LOG");
		if (env_level)
		{
			try
			{
				config.default_level = nano::log::parse_level (*env_level);

				std::cerr << "Using default log level from NANO_LOG environment variable: " << *env_level << std::endl;
			}
			catch (std::invalid_argument const & ex)
			{
				std::cerr << "Invalid log level from NANO_LOG environment variable: " << ex.what () << std::endl;
			}
		}

		// Parse per logger levels from environment variable, e.g. "NANO_LOG_LEVELS=ledger=debug,node=trace"
		auto env_levels = nano::get_env ("NANO_LOG_LEVELS");
		if (env_levels)
		{
			std::map<nano::log_config::logger_id_t, nano::log::level> levels;
			for (auto const & env_level_str : nano::util::split (*env_levels, ','))
			{
				try
				{
					// Split 'logger_name=level' into a pair of 'logger_name' and 'level'
					auto arr = nano::util::split (env_level_str, '=');
					if (arr.size () != 2)
					{
						throw std::invalid_argument ("Invalid entry: " + env_level_str);
					}

					auto name_str = arr[0];
					auto level_str = arr[1];

					auto logger_id = nano::log_config::parse_logger_id (name_str);
					auto logger_level = nano::log::parse_level (level_str);

					levels[logger_id] = logger_level;

					std::cerr << "Using logger log level from NANO_LOG_LEVELS environment variable: " << name_str << "=" << level_str << std::endl;
				}
				catch (std::invalid_argument const & ex)
				{
					std::cerr << "Invalid log level from NANO_LOG_LEVELS environment variable: " << ex.what () << std::endl;
				}
			}

			// Merge with existing levels
			for (auto const & [logger_id, level] : levels)
			{
				config.levels[logger_id] = level;
			}
		}

		return config;
	}
	catch (std::runtime_error const & ex)
	{
		std::cerr << "Unable to load log config. Using defaults. Error: " << ex.what () << std::endl;
	}
	return fallback;
}

void nano::log_with_rust (nano::log::level level, nano::log::type tag, const char * message, std::size_t size)
{
	rsnano::rsn_log (static_cast<uint8_t> (level), message, size);
}
