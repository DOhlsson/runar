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
"
);

#[derive(Debug)]
pub struct Options {
    pub exit: bool,
    pub error: bool,
    pub recursive: bool,
    pub verbose: bool,
    pub kill_timer: i32,
    pub command: Vec<OsString>,
    pub files: Vec<OsString>,
    pub sigmask: SigSet,
}

pub fn parse_args() -> Result<Options, ExitCode> {
    let args: Vec<OsString> = std::env::args_os().collect();

    let (mut args, command) = match args.iter().position(|arg| arg == "--") {
        Some(split_index) => {
            let (left, right) = args.split_at(split_index);
            (Vec::from(left), Some(Vec::from(right)))
        }
        None => (args.clone(), None),
    };

    args.remove(0); // Remove program name

    let mut args = Arguments::from_vec(args);

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

    if command.is_none() {
        eprintln!("<runar> Error: Expected command after -- argument");
        println!("{}", HELP);
        return Err(ExitCode::FAILURE);
    }

    let mut command = command.unwrap();
    command.remove(0); // Remove --

    if command.is_empty() {
        eprintln!("<runar> Error: Expected command after -- argument");
        println!("{}", HELP);
        return Err(ExitCode::FAILURE);
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

    let mut files = Vec::new();

    loop {
        match args.opt_value_from_str(["-f", "--file"]) {
            Ok(None) => break,
            Ok(Some(file)) => files.push(file),
            Err(e) => {
                eprintln!("<runar> Error: {}", e);
                return Err(ExitCode::FAILURE);
            }
        };
    }

    let remaining = args.finish();

    if !remaining.is_empty() {
        eprintln!("<runar> Error: Unknown arguments {:?}", remaining);
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
        command,
        files,
        sigmask,
    })
}
