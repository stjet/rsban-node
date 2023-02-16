#!/bin/bash
if build/core_test "$@"
then
	notify-send -i face-smile "Core Tests Passed!"
	play -q sounds/success.ogg
else 
	notify-send -i face-worried "Core Tests Failed!"
	play -q sounds/failed.ogg
fi
