#!/bin/bash
if cargo test --lib "$@"
then
	play -q ../sounds/success.ogg
else 
	play -q ../sounds/failed.ogg
fi
