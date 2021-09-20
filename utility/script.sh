#!/bin/sh

echo "starting $$"

if [ "$1" = "-t" ]; then
  echo Trapping SIGTERM and SIGINT
  trap '' TERM INT
fi

if [ "$1" = "-c" ]; then
  echo Not implemented
  # TODO: crash
fi

sleep 30s &
wait $!

echo "done $$"
