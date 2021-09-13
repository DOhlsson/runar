# runar
Watches for changes in files and runs/restarts a program. A resource-efficient replacement for nodemon

# Goals
To have a binary that is container friendly, dependency-free and of small size

# TODO

### features
* more tests
* trap sigterm & sigint and send it to prog
* timer sigterm -> sigkill
* exclude files
* better logging function
* handle stdin
* more cli options
* multi restart backoff
* minirunar more like a  C program, to get real small
* improve config handling, moving opt\_files looks weird
* trap sigchld instead of waiting for child process?

### bugs
* some weird sigterm bug
* kill attempts to kill killed process and if change happens while process is restarting

### optimization
* use different cli library, clap is too bloated
* use libc? =P
* analyze binarys sections for size hogs

# Size Milestones
- vim 3.1MB
- bash 1.2MB
- xterm 832KB
- ssh 780KB
- busybox 700KB
- **runar 576KB**
- tar 520KB
- nano 344KB
- find 304KB
- make 236KB
- grep 200KB
- less 172KB
- cp, mv, ls 144KB
- dash 124KB
- bc 96KB
- chmod 64KB
- head 48KB
- kill, inotifywait 32KB
