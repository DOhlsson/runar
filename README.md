# runar
Watches for changes in files and runs/restarts a program. A resource-efficient replacement for nodemon

# Goals
To have a barebones feature-set and while being under 100KB binary size

# TODO
* handle stdin
* more tests
* cli options
* use different cli library, clap is too bloated
* multi restart backoff
* fix bug where kill gets error while process is restarting
