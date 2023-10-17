#!/bin/bash

# read FFI function names
ffi_funcs=`rg --no-line-number --color never '^.* (rsn_[a-z_]+)[ \(].*$' --replace '$1' nano/lib/rsnano.hpp`

for func in $ffi_funcs
do
	occurences=`rg --count "$func" nano | wc -l`
	if [[ $occurences -lt 2 ]]
	then
		echo $func
	fi
done
