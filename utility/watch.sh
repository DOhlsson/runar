#!/bin/sh
watch -n0.1 'for r in $(pgrep runar) $(pgrep sleep) $(pgrep cargo); do echo -n "Parent: "; ps --no-header -o ppid -p $r; pstree -pg $r; done'
