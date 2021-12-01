#include <nano/lib/config.hpp>
#include <nano/lib/rsnano.hpp>
#include <nano/secure/utility.hpp>
#include <nano/secure/working.hpp>

#include <boost/filesystem.hpp>

boost::filesystem::path nano::working_path (nano::networks network)
{
	uint8_t buffer[256];
	int len = rsnano::rsn_working_path (static_cast<uint16_t> (network), buffer, sizeof (buffer));
	if (len < 0)
		throw std::runtime_error ("could not get working path");
	std::string s (reinterpret_cast<const char *> (buffer), len);
	boost::filesystem::path result (s);
	return result;
}

boost::filesystem::path nano::unique_path (nano::networks network)
{
	uint8_t buffer[256];
	int len = rsnano::rsn_unique_path (static_cast<uint16_t> (network), buffer, sizeof (buffer));
	if (len < 0)
		throw std::runtime_error ("could not get unique path");
	std::string s (reinterpret_cast<const char *> (buffer), len);
	boost::filesystem::path result (s);
	return result;
}

void nano::remove_temporary_directories ()
{
	rsnano::rsn_remove_temporary_directories ();
}

namespace nano
{
/** A wrapper for handling signals */
std::function<void ()> signal_handler_impl;
void signal_handler (int sig)
{
	if (signal_handler_impl != nullptr)
	{
		signal_handler_impl ();
	}
}
}
