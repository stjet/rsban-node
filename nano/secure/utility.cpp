#include <nano/lib/config.hpp>
#include <nano/lib/rsnano.hpp>
#include <nano/secure/utility.hpp>
#include <nano/secure/working.hpp>

#include <boost/system/error_code.hpp>

std::filesystem::path nano::working_path (nano::networks network)
{
	uint8_t buffer[256];
	int len = rsnano::rsn_working_path (static_cast<uint16_t> (network), buffer, sizeof (buffer));
	if (len < 0)
		throw std::runtime_error ("could not get working path");
	std::string s (reinterpret_cast<const char *> (buffer), len);
	std::filesystem::path result (s);
	return result;
}

std::filesystem::path nano::unique_path (nano::networks network)
{
	uint8_t buffer[256];
	int len = rsnano::rsn_unique_path (static_cast<uint16_t> (network), buffer, sizeof (buffer));
	if (len < 0)
		throw std::runtime_error ("could not get unique path");
	std::string s (reinterpret_cast<const char *> (buffer), len);
	std::filesystem::path result (s);
	return result;
}

void nano::remove_temporary_directories ()
{
	rsnano::rsn_remove_temporary_directories ();
}
