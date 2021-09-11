# runar
Watches for changes in files and runs/restarts a program. A resource-efficient replacement for nodemon

# Goals
To have a binary that is container friendly, dependency-free and of small size

# TODO

### features
* recursion
* exclude files
* catch sigterm and send it to prog
* more info with verbose flag
* better logging function
* handle stdin
* more tests
* more cli options
* multi restart backoff
* timer sigterm -> sigkill
* minirunar more like a  C program, to get real small
* improve config handling, moving opt\_files gets weird

### bugs
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
- *runar 560KB*
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
