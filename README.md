# runar
Watches for changes in files and RUNs And Restarts a program. A resource-efficient replacement for nodemon.

Quick links:
* [Features](#features)
* [Installation](#installation)
* [How to use](#how-to-use)
* [Bugs](#bugs)

# Features
* A binary that is container friendly, only depends on libc and is very small.
* Can be used to repeat commands until success/failure.

# Installation
Linux:
```shell
$ cargo install --git https://github.com/DOhlsson/runar.git
```

Docker:
```Dockerfile
ADD https://github.com/DOhlsson/runar/releases/download/0.2.0/runar /usr/bin/runar
RUN chmod a+x /usr/bin/runar
```

# How to use
```
$ runar -h
USAGE:
    runar [FLAGS] -- <COMMAND> <ARGS...>

FLAGS:
    -x, --exit                      exit when COMMAND returns zero
    -e, --exit-on-error             exits with the same status code as COMMAND
    -h, --help                      Prints help information
    -r, --recursive                 recursively watch directories
    -V, --version                   Prints version information
    -v, --verbose                   increases the level of verbosity
    -k, --kill-timer <kill-timer>   time in milliseconds until kill signal is sent [default: 5000]
    -f, --file <filename>           path to file or directory to watch, multiple flags allowed

ARGS:
    <COMMAND>    the COMMAND to execute
    <ARGS>...    the arguments to COMMAND
```

Watch a directory recursively and restart your program when the directory is updated.
```shell
$ runar -r -f ./src -- your program
```

Run and restart a program until it is successfull.
```shell
$ runar -x -- your program
```

More options are available, see the -h flag.

# Bugs
* Currently the target program will get paused by the system if it attempts to read stdin.
