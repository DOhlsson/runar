# TODO

### features
* exclude files
* timer sigterm -> sigkill
  * flag
* more tests
* dig deeper into signalfd and epoll
* trap sigterm & sigint and send it to prog
  * second sig should force kill on children and exit
* better logging function
* handle stdin
* multi restart backoff
* minirunar more like a  C program, to get real small
* improve config handling, moving opt\_files looks weird
* trap sigchld instead of waiting for child process?
* build.rs, escargot and a example test binary
* support alpine
* have a great big think on how exit codes should be handled in all cases

### bugs
* runar is too eager to start new process. it should wait until all children have exited
* kill attempts to kill killed process if change happens while process is restarting

### optimization
* use different cli library, clap is too bloated
* use libc directly?
* analyze binarys sections for size hogs
* use nix's inotify
