#!/bin/bash
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
if cargo test --lib -q "$@"
then
	play -q $SCRIPT_DIR/sounds/success.ogg&
else 
	play -q $SCRIPT_DIR/sounds/failed.ogg&
fi
