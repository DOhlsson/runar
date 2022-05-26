# TODO

### features
* handle runar getting SIGTERM
* rerun on change
* exclude files
* more tests
  * try to replicate kill of dead process
  * test for when process exits cleanly shortly after SIGTERM
* handle out of inotify instances
* dig deeper into signalfd and epoll
* trap sigterm & sigint and send it to prog
  * second sig should force kill on children and exit
* handle/detect loops in fs
* better logging function
* handle stdin
* multi restart backoff
* minirunar more like a  C program, to get real small
* trap sigchld instead of waiting for child process?
* support alpine
* have a great big think on how exit codes should be handled in all cases
* manpage
* cargo release
* replace walkdir?

### bugs
* kill attempts to kill dead process if change happens while process is restarting

### optimization
* analyze binarys sections for size hogs
