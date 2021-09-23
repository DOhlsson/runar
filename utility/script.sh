#!/bin/sh

while getopts "cps:t" o; do
  case "$o" in
    c) opt_crash=1;;
    p) opt_print_pid=1;;
    s) opt_sleep_time=$OPTARG;;
    t) opt_trap_sigs=1;;
    *)
      echo "Usage: $0 [-c] [-p] [-s secs] [-t]"
      exit 1
      ;;
  esac
done

sleep_time=${opt_sleep_time:-10}

if [ $opt_crash ]; then
  exit 1
fi

if [ $opt_print_pid ]; then
  printpid=$$
fi

if [ $opt_trap_sigs ]; then
  trap '' TERM INT
fi

echo start $printpid

sleep $sleep_time &
wait $!

echo end $printpid
