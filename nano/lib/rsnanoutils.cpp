#include <nano/boost/asio/ip/tcp.hpp>
#include <nano/lib/rsnanoutils.hpp>

boost::system::error_code rsnano::dto_to_error_code (rsnano::ErrorCodeDto const & dto)
{
	boost::system::error_category const * cat;
	if (dto.category == 0)
	{
		cat = &boost::system::generic_category ();
	}
	else
	{
		cat = &boost::system::system_category ();
	}

	return boost::system::error_code (dto.val, *cat);
}

rsnano::ErrorCodeDto rsnano::error_code_to_dto (boost::system::error_code const & ec)
{
	rsnano::ErrorCodeDto dto;
	dto.val = ec.value ();
	if (ec.category () == boost::system::generic_category ())
	{
		dto.category = 0;
	}
	else
	{
		dto.category = 1;
	}

	return dto;
}

rsnano::EndpointDto rsnano::endpoint_to_dto (boost::asio::ip::tcp::endpoint const & ep)
{
	rsnano::EndpointDto dto;
	dto.port = ep.port ();
	dto.v6 = ep.address ().is_v6 ();
	if (dto.v6)
	{
		auto bytes{ ep.address ().to_v6 ().to_bytes () };
		std::copy (std::begin (bytes), std::end (bytes), std::begin (dto.bytes));
	}
	else
	{
		auto bytes{ ep.address ().to_v4 ().to_bytes () };
		std::copy (std::begin (bytes), std::end (bytes), std::begin (dto.bytes));
	}
	return dto;
}

boost::asio::ip::tcp::endpoint rsnano::dto_to_endpoint (rsnano::EndpointDto const & dto)
{
	if (dto.v6)
	{
		std::array<unsigned char, 16> bytes;
		std::copy (std::begin (dto.bytes), std::end (dto.bytes), std::begin (bytes));
		boost::asio::ip::address_v6 addr{ bytes };
		return boost::asio::ip::tcp::endpoint (boost::asio::ip::address{ addr }, dto.port);
	}

	std::array<unsigned char, 4> bytes;
	std::copy (dto.bytes, dto.bytes + 4, std::begin (bytes));
	boost::asio::ip::address_v4 addr{ bytes };
	return boost::asio::ip::tcp::endpoint (boost::asio::ip::address{ addr }, dto.port);
}
