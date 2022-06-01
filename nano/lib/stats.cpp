#include <nano/lib/jsonconfig.hpp>
#include <nano/lib/locks.hpp>
#include <nano/lib/stats.hpp>
#include <nano/lib/tomlconfig.hpp>

#include <boost/format.hpp>
#include <boost/property_tree/json_parser.hpp>

#include <ctime>
#include <fstream>
#include <sstream>

void nano::stat_config::load_dto (rsnano::StatConfigDto & dto)
{
	sampling_enabled = dto.sampling_enabled;
	capacity = dto.capacity;
	interval = dto.interval;
	log_interval_samples = dto.log_interval_samples;
	log_interval_counters = dto.log_interval_counters;
	log_rotation_count = dto.log_rotation_count;
	log_headers = dto.log_headers;
	log_counters_filename = std::string (reinterpret_cast<const char *> (dto.log_counters_filename), dto.log_counters_filename_len);
	log_samples_filename = std::string (reinterpret_cast<const char *> (dto.log_samples_filename), dto.log_samples_filename_len);
}

rsnano::StatConfigDto nano::stat_config::to_dto () const
{
	rsnano::StatConfigDto dto;
	dto.sampling_enabled = sampling_enabled;
	dto.capacity = capacity;
	dto.interval = interval;
	dto.log_interval_samples = log_interval_samples;
	dto.log_interval_counters = log_interval_counters;
	dto.log_rotation_count = log_rotation_count;
	dto.log_headers = log_headers;
	std::copy (log_counters_filename.begin (), log_counters_filename.end (), std::begin (dto.log_counters_filename));
	dto.log_counters_filename_len = log_counters_filename.size ();

	std::copy (log_samples_filename.begin (), log_samples_filename.end (), std::begin (dto.log_samples_filename));
	dto.log_samples_filename_len = log_samples_filename.size ();
	return dto;
}

nano::error nano::stat_config::deserialize_toml (nano::tomlconfig & toml)
{
	auto sampling_l (toml.get_optional_child ("sampling"));
	if (sampling_l)
	{
		sampling_l->get<bool> ("enable", sampling_enabled);
		sampling_l->get<size_t> ("capacity", capacity);
		sampling_l->get<size_t> ("interval", interval);
	}

	auto log_l (toml.get_optional_child ("log"));
	if (log_l)
	{
		log_l->get<bool> ("headers", log_headers);
		log_l->get<size_t> ("interval_counters", log_interval_counters);
		log_l->get<size_t> ("interval_samples", log_interval_samples);
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

std::string nano::tm_to_string (tm & tm)
{
	return (boost::format ("%04d.%02d.%02d %02d:%02d:%02d") % (1900 + tm.tm_year) % (tm.tm_mon + 1) % tm.tm_mday % tm.tm_hour % tm.tm_min % tm.tm_sec).str ();
}

nano::stat_log_sink::stat_log_sink (rsnano::StatLogSinkHandle * handle_a) :
	handle (handle_a)
{
}

nano::stat_log_sink::~stat_log_sink ()
{
	rsnano::rsn_stat_log_sink_destroy (handle);
}

void nano::stat_log_sink::begin ()
{
	rsnano::rsn_stat_log_sink_begin (handle);
}

void nano::stat_log_sink::finalize ()
{
	rsnano::rsn_stat_log_sink_finalize (handle);
}

void nano::stat_log_sink::write_header (std::string const & header, std::chrono::system_clock::time_point & walltime)
{
	rsnano::rsn_stat_log_sink_write_header (handle, header.c_str (), std::chrono::duration_cast<std::chrono::milliseconds> (walltime.time_since_epoch ()).count ());
}

void nano::stat_log_sink::write_entry (std::chrono::system_clock::time_point & time, std::string const & type, std::string const & detail, std::string const & dir, uint64_t value, nano::stat_histogram * histogram)
{
	rsnano::StatHistogramHandle * hist_handle = nullptr;
	if (histogram != nullptr)
	{
		hist_handle = histogram->handle;
	}
	rsnano::rsn_stat_log_sink_write_entry (handle, std::chrono::duration_cast<std::chrono::milliseconds> (time.time_since_epoch ()).count (), type.c_str (), detail.c_str (), dir.c_str (), value, hist_handle);
}

void nano::stat_log_sink::rotate ()
{
	rsnano::rsn_stat_log_sink_rotate (handle);
}

size_t nano::stat_log_sink::entries ()
{
	return rsnano::rsn_stat_log_sink_entries (handle);
}

void nano::stat_log_sink::inc_entries ()
{
	rsnano::rsn_stat_log_sink_inc_entries (handle);
}

std::string nano::stat_log_sink::to_string ()
{
	rsnano::StringDto dto;
	rsnano::rsn_stat_log_sink_to_string (handle, &dto);
	std::string result (dto.value);
	rsnano::rsn_string_destroy (dto.handle);
	return result;
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

nano::stat_histogram::stat_histogram (std::initializer_list<uint64_t> intervals_a, size_t bin_count_a)
{
	std::vector<uint64_t> intervals_l{ intervals_a };
	handle = rsnano::rsn_stat_histogram_create (intervals_l.data (), intervals_l.size (), bin_count_a);
}

nano::stat_histogram::stat_histogram (rsnano::StatHistogramHandle * handle) :
	handle{ handle }
{
}

nano::stat_histogram::stat_histogram (nano::stat_histogram const & other_a) :
	handle{ rsnano::rsn_stat_histogram_clone (other_a.handle) }
{
}

nano::stat_histogram::stat_histogram (nano::stat_histogram && other_a) :
	handle{ other_a.handle }
{
	other_a.handle = nullptr;
}

nano::stat_histogram::~stat_histogram ()
{
	if (handle != nullptr)
	{
		rsnano::rsn_stat_histogram_destroy (handle);
	}
}

void nano::stat_histogram::add (uint64_t index_a, uint64_t addend_a)
{
	rsnano::rsn_stat_histogram_add (handle, index_a, addend_a);
}

std::vector<nano::stat_histogram::bin> nano::stat_histogram::get_bins () const
{
	rsnano::HistogramBinsDto bins_dto;
	rsnano::rsn_stat_histogram_get_bins (handle, &bins_dto);
	std::vector<nano::stat_histogram::bin> bins;
	bins.reserve (bins_dto.len);
	for (auto i = 0; i < bins_dto.len; ++i)
	{
		rsnano::HistogramBinDto const * bin_dto = bins_dto.bins + i;
		nano::stat_histogram::bin bin{ bin_dto->start_inclusive, bin_dto->end_exclusive };
		bin.value = bin_dto->value;
		bin.timestamp = std::chrono::system_clock::time_point (std::chrono::milliseconds (bin_dto->timestamp_ms));
		bins.push_back (bin);
	}
	return bins;
}

nano::stat_entry::stat_entry (size_t capacity, size_t interval) :
	handle (rsnano::rsn_stat_entry_create (capacity, interval))
{
}

nano::stat_entry::~stat_entry ()
{
	rsnano::rsn_stat_entry_destroy (handle);
}

size_t nano::stat_entry::get_sample_interval ()
{
	return rsnano::rsn_stat_entry_get_sample_interval (handle);
}

void nano::stat_entry::set_sample_interval (size_t interval)
{
	rsnano::rsn_stat_entry_set_sample_interval (handle, interval);
}

void nano::stat_entry::sample_current_add (uint64_t value, bool update_timestamp)
{
	rsnano::rsn_stat_entry_sample_current_add (handle, value, update_timestamp);
}

void nano::stat_entry::sample_current_set_value (uint64_t value)
{
	rsnano::rsn_stat_entry_sample_current_set_value (handle, value);
}

void nano::stat_entry::sample_current_set_timestamp (std::chrono::system_clock::time_point value)
{
	rsnano::rsn_stat_entry_sample_current_set_timestamp (handle, std::chrono::duration_cast<std::chrono::milliseconds> (value.time_since_epoch ()).count ());
}

void nano::stat_entry::add_sample (nano::stat_datapoint const & sample)
{
	rsnano::rsn_stat_entry_add_sample (handle, sample.handle);
}

uint64_t nano::stat_entry::get_counter_value ()
{
	return rsnano::rsn_stat_entry_get_counter_value (handle);
}

std::chrono::system_clock::time_point nano::stat_entry::get_counter_timestamp ()
{
	std::chrono::milliseconds ms (rsnano::rsn_stat_entry_get_counter_timestamp (handle));
	return std::chrono::system_clock::time_point (ms);
}

void nano::stat_entry::counter_add (uint64_t addend, bool update_timestamp)
{
	rsnano::rsn_stat_entry_counter_add (handle, addend, update_timestamp);
}

void nano::stat_entry::define_histogram (std::initializer_list<uint64_t> intervals_a, size_t bin_count_a)
{
	std::vector<uint64_t> intervals_l{ intervals_a };
	rsnano::rsn_stat_entry_define_histogram (handle, intervals_l.data (), intervals_l.size (), bin_count_a);
}

void nano::stat_entry::update_histogram (uint64_t index_a, uint64_t addend_a)
{
	rsnano::rsn_stat_entry_update_histogram (handle, index_a, addend_a);
}

nano::stat_histogram nano::stat_entry::get_histogram () const
{
	auto histogram = rsnano::rsn_stat_entry_get_histogram (handle);
	debug_assert (histogram != nullptr);
	return nano::stat_histogram{ histogram };
}

std::chrono::system_clock::time_point nano::stat_entry::get_sample_start_time ()
{
	auto ms{ std::chrono::milliseconds (rsnano::rsn_stat_entry_get_sample_start_time (handle)) };
	return std::chrono::system_clock::time_point (ms);
}

void nano::stat_entry::set_sample_start_time (std::chrono::system_clock::time_point time)
{
	rsnano::rsn_stat_entry_set_sample_start_time (handle, std::chrono::duration_cast<std::chrono::milliseconds> (time.time_since_epoch ()).count ());
}

nano::stat_datapoint nano::stat_entry::sample_current ()
{
	return nano::stat_datapoint{ rsnano::rsn_stat_entry_sample_current (handle) };
}

std::vector<nano::stat_datapoint> nano::stat_entry::get_samples ()
{
	auto count = rsnano::rsn_stat_entry_get_sample_count (handle);
	std::vector<nano::stat_datapoint> result;
	result.reserve (count);
	for (auto i = 0; i < count; ++i)
	{
		result.emplace_back (rsnano::rsn_stat_entry_get_sample (handle, i));
	}
	return result;
}

nano::stat::stat () :
	stat (nano::stat_config ())
{
}

nano::stat::stat (nano::stat_config config) :
	config (config)
{
	auto config_dto{ config.to_dto () };
	handle = rsnano::rsn_stat_create (&config_dto);
}

nano::stat::~stat ()
{
	rsnano::rsn_stat_destroy (handle);
}

std::shared_ptr<nano::stat_entry> nano::stat::get_entry (uint32_t key)
{
	return get_entry (key, config.interval, config.capacity);
}

std::shared_ptr<nano::stat_entry> nano::stat::get_entry (uint32_t key, size_t interval, size_t capacity)
{
	nano::unique_lock<nano::mutex> lock (stat_mutex);
	return get_entry_impl (key, interval, capacity);
}

std::shared_ptr<nano::stat_entry> nano::stat::get_entry_impl (uint32_t key, size_t interval, size_t capacity)
{
	std::shared_ptr<nano::stat_entry> res;
	auto entry = entries.find (key);
	if (entry == entries.end ())
	{
		res = entries.emplace (key, std::make_shared<nano::stat_entry> (capacity, interval)).first->second;
	}
	else
	{
		res = entry->second;
	}

	return res;
}

std::unique_ptr<nano::stat_log_sink> nano::stat::log_sink_json () const
{
	return std::make_unique<json_writer> ();
}

void nano::stat::log_counters (stat_log_sink & sink)
{
	nano::unique_lock<nano::mutex> lock (stat_mutex);
	log_counters_impl (sink);
}

void nano::stat::log_counters_impl (stat_log_sink & sink)
{
	sink.begin ();
	if (sink.entries () >= config.log_rotation_count)
	{
		sink.rotate ();
	}

	if (config.log_headers)
	{
		auto walltime (std::chrono::system_clock::now ());
		sink.write_header ("counters", walltime);
	}

	for (auto & it : entries)
	{
		auto time = it.second->get_counter_timestamp ();
		auto key = it.first;
		std::string type = type_to_string (key);
		std::string detail = detail_to_string (key);
		std::string dir = dir_to_string (key);
		auto histogram{ it.second->get_histogram () };
		sink.write_entry (time, type, detail, dir, it.second->get_counter_value (), &histogram);
	}
	sink.inc_entries ();
	sink.finalize ();
}

void nano::stat::log_samples (stat_log_sink & sink)
{
	nano::unique_lock<nano::mutex> lock (stat_mutex);
	log_samples_impl (sink);
}

void nano::stat::log_samples_impl (stat_log_sink & sink)
{
	sink.begin ();
	if (sink.entries () >= config.log_rotation_count)
	{
		sink.rotate ();
	}

	if (config.log_headers)
	{
		auto walltime (std::chrono::system_clock::now ());
		sink.write_header ("samples", walltime);
	}

	for (auto & it : entries)
	{
		auto key = it.first;
		std::string type = type_to_string (key);
		std::string detail = detail_to_string (key);
		std::string dir = dir_to_string (key);

		for (auto & datapoint : it.second->get_samples ())
		{
			auto time = datapoint.get_timestamp ();
			sink.write_entry (time, type, detail, dir, datapoint.get_value (), nullptr);
		}
	}
	sink.inc_entries ();
	sink.finalize ();
}

void nano::stat::define_histogram (stat::type type, stat::detail detail, stat::dir dir, std::initializer_list<uint64_t> intervals_a, size_t bin_count_a /*=0*/)
{
	auto entry (get_entry (key_of (type, detail, dir)));
	entry->define_histogram (intervals_a, bin_count_a);
}

void nano::stat::update_histogram (stat::type type, stat::detail detail, stat::dir dir, uint64_t index_a, uint64_t addend_a)
{
	auto entry (get_entry (key_of (type, detail, dir)));
	entry->update_histogram (index_a, addend_a);
}

nano::stat_histogram nano::stat::get_histogram (stat::type type, stat::detail detail, stat::dir dir)
{
	auto entry (get_entry (key_of (type, detail, dir)));
	return entry->get_histogram ();
}

void nano::stat::update (uint32_t key_a, uint64_t value)
{
	static file_writer log_count (config.log_counters_filename);
	static file_writer log_sample (config.log_samples_filename);

	auto now (std::chrono::steady_clock::now ());
	auto now2 (std::chrono::system_clock::now ());

	nano::unique_lock<nano::mutex> lock (stat_mutex);
	if (!stopped)
	{
		auto entry (get_entry_impl (key_a, config.interval, config.capacity));

		// Counters
		auto old (entry->get_counter_value ());
		entry->counter_add (value);

		std::chrono::duration<double, std::milli> duration = now - log_last_count_writeout;
		if (config.log_interval_counters > 0 && duration.count () > config.log_interval_counters)
		{
			log_counters_impl (log_count);
			log_last_count_writeout = now;
		}

		// Samples
		if (config.sampling_enabled && entry->get_sample_interval () > 0)
		{
			entry->sample_current_add (value, false);

			std::chrono::duration<double, std::milli> duration = now2 - entry->get_sample_start_time ();
			if (duration.count () > entry->get_sample_interval ())
			{
				entry->set_sample_start_time (now2);

				// Make a snapshot of samples for thread safety and to get a stable container
				entry->sample_current_set_timestamp (std::chrono::system_clock::now ());
				entry->add_sample (entry->sample_current ());
				entry->sample_current_set_value (0);

				// Log sink
				duration = now - log_last_sample_writeout;
				if (config.log_interval_samples > 0 && duration.count () > config.log_interval_samples)
				{
					log_samples_impl (log_sample);
					log_last_sample_writeout = now;
				}
			}
		}
	}
}

std::chrono::seconds nano::stat::last_reset ()
{
	nano::unique_lock<nano::mutex> lock (stat_mutex);
	auto now (std::chrono::steady_clock::now ());
	return std::chrono::duration_cast<std::chrono::seconds> (now - timestamp);
}

void nano::stat::stop ()
{
	nano::lock_guard<nano::mutex> guard (stat_mutex);
	stopped = true;
}

void nano::stat::clear ()
{
	nano::unique_lock<nano::mutex> lock (stat_mutex);
	entries.clear ();
	timestamp = std::chrono::steady_clock::now ();
}

std::string nano::stat::type_to_string (uint32_t key)
{
	uint8_t const * ptr;
	auto len = rsnano::rsn_stat_type_to_string (key, &ptr);
	return std::string (reinterpret_cast<const char *> (ptr), len);
}

std::string nano::stat::detail_to_string (stat::detail detail)
{
	uint8_t const * ptr;
	auto len = rsnano::rsn_stat_type_to_string (static_cast<uint8_t> (detail), &ptr);
	return std::string (reinterpret_cast<const char *> (ptr), len);
}

std::string nano::stat::detail_to_string (uint32_t key)
{
	uint8_t const * ptr;
	auto len = rsnano::rsn_stat_detail_to_string (key, &ptr);
	return std::string (reinterpret_cast<const char *> (ptr), len);
}

std::string nano::stat::dir_to_string (uint32_t key)
{
	auto dir = static_cast<stat::dir> (key & 0x000000ff);
	std::string res;
	switch (dir)
	{
		case nano::stat::dir::in:
			res = "in";
			break;
		case nano::stat::dir::out:
			res = "out";
			break;
	}
	return res;
}

void nano::stat::configure (stat::type type, stat::detail detail, stat::dir dir, size_t interval, size_t capacity)
{
	get_entry (key_of (type, detail, dir), interval, capacity);
}

void nano::stat::disable_sampling (stat::type type, stat::detail detail, stat::dir dir)
{
	auto entry = get_entry (key_of (type, detail, dir));
	entry->set_sample_interval (0);
}

void nano::stat::inc (stat::type type, stat::dir dir)
{
	add (type, dir, 1);
}

void nano::stat::inc_detail_only (stat::type type, stat::detail detail, stat::dir dir)
{
	add (type, detail, dir, 1, true);
}

void nano::stat::inc (stat::type type, stat::detail detail, stat::dir dir)
{
	add (type, detail, dir, 1);
}

void nano::stat::add (stat::type type, stat::dir dir, uint64_t value)
{
	add (type, detail::all, dir, value);
}

void nano::stat::add (stat::type type, stat::detail detail, stat::dir dir, uint64_t value, bool detail_only)
{
	if (value == 0)
	{
		return;
	}

	constexpr uint32_t no_detail_mask = 0xffff00ff;
	uint32_t key = key_of (type, detail, dir);

	update (key, value);

	// Optionally update at type-level as well
	if (!detail_only && (key & no_detail_mask) != key)
	{
		update (key & no_detail_mask, value);
	}
}

uint64_t nano::stat::count (stat::type type, stat::dir dir)
{
	return count (type, stat::detail::all, dir);
}

uint64_t nano::stat::count (stat::type type, stat::detail detail, stat::dir dir)
{
	return get_entry (key_of (type, detail, dir))->get_counter_value ();
}

nano::stat_datapoint::stat_datapoint () :
	handle (rsnano::rsn_stat_datapoint_create ())
{
}

nano::stat_datapoint::~stat_datapoint ()
{
	rsnano::rsn_stat_datapoint_destroy (handle);
}

nano::stat_datapoint::stat_datapoint (stat_datapoint const & other_a)
{
	handle = rsnano::rsn_stat_datapoint_clone (other_a.handle);
}

nano::stat_datapoint::stat_datapoint (rsnano::StatDatapointHandle * handle) :
	handle{ handle }
{
}

nano::stat_datapoint & nano::stat_datapoint::operator= (stat_datapoint const & other_a)
{
	handle = rsnano::rsn_stat_datapoint_clone (other_a.handle);
	return *this;
}

uint64_t nano::stat_datapoint::get_value () const
{
	return rsnano::rsn_stat_datapoint_get_value (handle);
}

void nano::stat_datapoint::set_value (uint64_t value_a)
{
	rsnano::rsn_stat_datapoint_set_value (handle, value_a);
}

std::chrono::system_clock::time_point nano::stat_datapoint::get_timestamp () const
{
	auto timestamp_ms = rsnano::rsn_stat_datapoint_get_timestamp_ms (handle);
	return std::chrono::system_clock::time_point{ std::chrono::milliseconds (timestamp_ms) };
}

void nano::stat_datapoint::set_timestamp (std::chrono::system_clock::time_point timestamp_a)
{
	auto timestamp_ms (std::chrono::duration_cast<std::chrono::milliseconds> (timestamp_a.time_since_epoch ()));
	rsnano::rsn_stat_datapoint_set_timestamp_ms (handle, timestamp_ms.count ());
}

/** Add \addend to the current value and optionally update the timestamp */
void nano::stat_datapoint::add (uint64_t addend, bool update_timestamp)
{
	rsnano::rsn_stat_datapoint_add (handle, addend, update_timestamp);
}
