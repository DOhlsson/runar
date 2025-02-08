use std::ffi::OsString;
use std::process::ExitCode;

use nix::poll::PollTimeout;
use nix::sys::signal::{self, SigSet};

use pico_args::Arguments;

const HELP: &str = concat!(
    env!("CARGO_PKG_NAME"),
    " ",
    env!("CARGO_PKG_VERSION"),
    "\n\n",
    env!("CARGO_PKG_DESCRIPTION"),
    "\n\n",
    "\
USAGE:
    runar [FLAGS] -- <COMMAND> [ARGS...]

FLAGS:
    -f, --file <filename>           path to file or directory to watch, multiple flags allowed
    -r, --recursive                 recursively watch directories
    -e, --exit                      exit runar if COMMAND returns status code 0
    -E, --exit-on-error             exit runar if COMMAND returns statuse code >0
    -s, --restart                   restart COMMAND if it returns status code 0
    -S, --restart-on-error          restart COMMAND if it returns status code >0
    -k, --kill-timer <kill-timer>   time in milliseconds until kill signal is sent (default: 5000)
    -v, --verbose                   increases the level of verbosity
    -h, --help                      Prints help information

ARGS:
    <COMMAND>    the COMMAND to execute
    [ARGS...]    the arguments to COMMAND
"
);

pub struct Options {
    pub exit_on_zero: bool,
    pub exit_on_error: bool,
    pub restart_on_zero: bool,
    pub restart_on_error: bool,
    pub recursive: bool,
    pub verbose: bool,
    pub kill_timer: PollTimeout,
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
        println!("{HELP}");
        return Err(ExitCode::SUCCESS);
    }

    let Some(mut command) = command else {
        eprintln!("<runar> Error: Expected command after -- argument");
        println!("{HELP}");
        return Err(ExitCode::FAILURE);
    };

    command.remove(0); // Remove --

    if command.is_empty() {
        eprintln!("<runar> Error: Expected command after -- argument");
        println!("{HELP}");
        return Err(ExitCode::FAILURE);
    }

    let exit_on_zero = args.contains(["-e", "--exit"]);
    let exit_on_error = args.contains(["-E", "--exit-on-error"]);
    let restart_on_zero = args.contains(["-s", "--restart"]);
    let restart_on_error = args.contains(["-S", "--restart-on-error"]);
    let recursive = args.contains(["-r", "--recursive"]);
    let verbose = args.contains(["-v", "--verbose"]);

    let kill_timer = match args.opt_value_from_str::<_, i32>(["-k", "--kill-timer"]) {
        Ok(None) => PollTimeout::from(5000_u16),
        Ok(Some(kt)) => PollTimeout::try_from(kt).expect("polltimeout"),
        Err(e) => {
            eprintln!("<runar> Error: {e}");
            return Err(ExitCode::FAILURE);
        }
    };

    let mut files = Vec::new();

    loop {
        match args.opt_value_from_str(["-f", "--file"]) {
            Ok(None) => break,
            Ok(Some(file)) => files.push(file),
            Err(e) => {
                eprintln!("<runar> Error: {e}");
                return Err(ExitCode::FAILURE);
            }
        };
    }

    let remaining = args.finish();

    if !remaining.is_empty() {
        eprintln!("<runar> Error: Unknown arguments {remaining:?}");
        return Err(ExitCode::FAILURE);
    }

    // This might become configurable in the future
    let mut sigmask = SigSet::empty();
    sigmask.add(signal::SIGHUP);
    sigmask.add(signal::SIGINT);
    sigmask.add(signal::SIGTERM);
    sigmask.add(signal::SIGCHLD);

    Ok(Options {
        exit_on_zero,
        exit_on_error,
        restart_on_zero,
        restart_on_error,
        recursive,
        verbose,
        kill_timer,
        command,
        files,
        sigmask,
    })
}
