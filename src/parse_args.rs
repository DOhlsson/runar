use std::ffi::OsString;
use std::process::ExitCode;

use nix::sys::signal;
use nix::sys::signal::SigSet;

use pico_args::Arguments;

const HELP: &str = concat!(
    env!("CARGO_PKG_NAME"),
    " ",
    env!("CARGO_PKG_VERSION"),
    "\n",
    env!("CARGO_PKG_AUTHORS"),
    "\n",
    env!("CARGO_PKG_DESCRIPTION"),
    "\n\n",
    "\
USAGE:
    runar [FLAGS] [OPTIONS] <COMMAND> <FILE>...

FLAGS:
    -x, --exit                      exit when COMMAND returns zero
    -e, --exit-on-error             exits with the same status code as COMMAND
    -h, --help                      Prints help information
    -r, --recursive                 recursively watch directories
    -V, --version                   Prints version information
    -v, --verbose                   increases the level of verbosity
    -k, --kill-timer <kill-timer>   time in milliseconds until kill signal is sent [default: 5000]

ARGS:
    <COMMAND>    the COMMAND to execute
    <FILE>...    the file(s) to watch
"
);

#[derive(Debug)]
pub struct Options {
    pub exit: bool,
    pub error: bool,
    pub recursive: bool,
    pub verbose: bool,
    pub kill_timer: i32,
    pub command: OsString,
    pub files: Vec<OsString>,
    pub sigmask: SigSet,
}

pub fn parse_args() -> Result<Options, ExitCode> {
    let mut args = Arguments::from_env();

    if args.contains(["-h", "--help"]) {
        println!("{}", HELP);
        return Err(ExitCode::SUCCESS);
    }

    if args.contains(["-V", "--version"]) {
        println!(
            "{} version {}",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION")
        );
        return Err(ExitCode::SUCCESS);
    }

    let exit = args.contains(["-x", "--exit"]);
    let error = args.contains(["-e", "--exit-on-error"]);
    let recursive = args.contains(["-r", "--recursive"]);
    let verbose = args.contains(["-v", "--verbose"]);

    let kill_timer = match args.opt_value_from_str(["-k", "--kill-timer"]) {
        Ok(None) => 5000,
        Ok(Some(kt)) if kt >= 0 => kt,
        Ok(Some(_)) => {
            eprintln!("<runar> Error: kill timer must be a positive integer");
            return Err(ExitCode::FAILURE);
        }
        Err(e) => {
            eprintln!("<runar> Error: {}", e);
            return Err(ExitCode::FAILURE);
        }
    };

    let mut remaining = args.finish();

    if remaining.len() < 2 {
        eprintln!("Too few arguments");
        println!("\n{}", HELP);
        return Err(ExitCode::FAILURE);
    }

    // This might become configurable in the future
    let mut sigmask = SigSet::empty();
    sigmask.add(signal::SIGHUP);
    sigmask.add(signal::SIGINT);
    sigmask.add(signal::SIGTERM);
    sigmask.add(signal::SIGCHLD);

    Ok(Options {
        exit,
        error,
        recursive,
        verbose,
        kill_timer,
        command: remaining.remove(0),
        files: remaining,
        sigmask,
    })
}
