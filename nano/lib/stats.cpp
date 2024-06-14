#include <nano/lib/jsonconfig.hpp>
#include <nano/lib/locks.hpp>
#include <nano/lib/stats.hpp>
#include <nano/lib/tomlconfig.hpp>

#include <boost/format.hpp>
#include <boost/property_tree/json_parser.hpp>

#include <ctime>

void nano::stats_config::load_dto (rsnano::StatConfigDto & dto)
{
	max_samples = dto.max_samples;
	log_samples_interval = std::chrono::milliseconds{ dto.log_samples_interval };
	log_counters_interval = std::chrono::milliseconds{ dto.log_counters_interval };
	log_rotation_count = dto.log_rotation_count;
	log_headers = dto.log_headers;
	log_counters_filename = std::string (reinterpret_cast<const char *> (dto.log_counters_filename), dto.log_counters_filename_len);
	log_samples_filename = std::string (reinterpret_cast<const char *> (dto.log_samples_filename), dto.log_samples_filename_len);
}

rsnano::StatConfigDto nano::stats_config::to_dto () const
{
	rsnano::StatConfigDto dto{};
	dto.max_samples = max_samples;
	dto.log_samples_interval = log_samples_interval.count ();
	dto.log_counters_interval = log_counters_interval.count ();
	dto.log_rotation_count = log_rotation_count;
	dto.log_headers = log_headers;
	std::copy (log_counters_filename.begin (), log_counters_filename.end (), std::begin (dto.log_counters_filename));
	dto.log_counters_filename_len = log_counters_filename.size ();

	std::copy (log_samples_filename.begin (), log_samples_filename.end (), std::begin (dto.log_samples_filename));
	dto.log_samples_filename_len = log_samples_filename.size ();
	return dto;
}

nano::error nano::stats_config::deserialize_toml (nano::tomlconfig & toml)
{
	toml.get<size_t> ("max_samples", max_samples);

	auto log_l (toml.get_optional_child ("log"));
	if (log_l)
	{
		log_l->get<bool> ("headers", log_headers);
		auto counters_interval_l = log_counters_interval.count ();
		log_l->get<long> ("interval_counters", counters_interval_l);
		log_counters_interval = std::chrono::milliseconds{ counters_interval_l };

		auto samples_interval_l = log_samples_interval.count ();
		log_l->get<long> ("interval_samples", samples_interval_l);
		log_samples_interval = std::chrono::milliseconds{ samples_interval_l };

		log_l->get<size_t> ("rotation_count", log_rotation_count);
		log_l->get<std::string> ("filename_counters", log_counters_filename);
		log_l->get<std::string> ("filename_samples", log_samples_filename);

		// Don't allow specifying the same file name for counter and samples logs
		if (log_counters_filename == log_samples_filename)
		{
			toml.get_error ().set ("The statistics counter and samples config values must be different");
		}
	}

	return toml.get_error ();
}

nano::stat_log_sink::stat_log_sink (rsnano::StatLogSinkHandle * handle_a) :
	handle (handle_a)
{
}

nano::stat_log_sink::~stat_log_sink ()
{
	rsnano::rsn_stat_log_sink_destroy (handle);
}

void * nano::stat_log_sink::to_object ()
{
	return rsnano::rsn_stat_log_sink_to_object (handle);
}

/** JSON sink. The resulting JSON object is provided as both a property_tree::ptree (to_object) and a string (to_string) */
class json_writer : public nano::stat_log_sink
{
public:
	json_writer () :
		stat_log_sink (rsnano::rsn_json_writer_create ())
	{
	}
};

/** File sink with rotation support. This writes one counter per line and does not include histogram values. */
class file_writer : public nano::stat_log_sink
{
public:
	explicit file_writer (std::string const & filename_a) :
		stat_log_sink{ rsnano::rsn_file_writer_create (reinterpret_cast<const int8_t *> (filename_a.c_str ())) }
	{
	}
};

nano::stats::stats () :
	stats (nano::stats_config ())
{
}

nano::stats::stats (rsnano::StatHandle * handle_a) :
	handle{ handle_a }
{
}

nano::stats::stats (nano::stats_config config)
{
	auto config_dto{ config.to_dto () };
	handle = rsnano::rsn_stat_create (&config_dto);
}

nano::stats::~stats ()
{
	rsnano::rsn_stat_destroy (handle);
}

std::unique_ptr<nano::stat_log_sink> nano::stats::log_sink_json () const
{
	return std::make_unique<json_writer> ();
}

void nano::stats::log_counters (stat_log_sink & sink)
{
	rsnano::rsn_stat_log_counters (handle, sink.handle);
}

void nano::stats::log_samples (stat_log_sink & sink)
{
	rsnano::rsn_stat_log_samples (handle, sink.handle);
}

std::chrono::seconds nano::stats::last_reset ()
{
	return std::chrono::seconds{ rsnano::rsn_stat_last_reset_s (handle) };
}

void nano::stats::stop ()
{
	rsnano::rsn_stat_stop (handle);
}

void nano::stats::clear ()
{
	rsnano::rsn_stat_clear (handle);
}

void nano::stats::inc (stat::type type, stat::dir dir)
{
	add (type, dir, 1);
}

void nano::stats::inc (stat::type type, stat::detail detail, stat::dir dir)
{
	add (type, detail, dir, 1);
}

void nano::stats::add (stat::type type, stat::dir dir, uint64_t value)
{
	add (type, stat::detail::all, dir, value);
}

void nano::stats::add (stat::type type, stat::detail detail, stat::dir dir, uint64_t value)
{
	rsnano::rsn_stat_add (handle,
	static_cast<uint8_t> (type),
	static_cast<uint16_t> (detail),
	static_cast<uint8_t> (dir),
	value);
}

uint64_t nano::stats::count (stat::type type, stat::dir dir)
{
	return rsnano::rsn_stat_count_all (handle,
	static_cast<uint8_t> (type),
	static_cast<uint8_t> (dir));
}

uint64_t nano::stats::count (stat::type type, stat::detail detail, stat::dir dir)
{
	return rsnano::rsn_stat_count (handle,
	static_cast<uint8_t> (type),
	static_cast<uint16_t> (detail),
	static_cast<uint8_t> (dir));
}
