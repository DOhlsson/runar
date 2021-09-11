use std::process;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use nix::sys::signal::{kill, SIGTERM};
use nix::unistd::Pid;

use inotify::{Inotify, WatchMask};

use clap::{App, Arg};

const VERSION: &'static str = env!("CARGO_PKG_VERSION");
const AUTHORS: &'static str = env!("CARGO_PKG_AUTHORS");
const DESCRIPTION: &'static str = env!("CARGO_PKG_DESCRIPTION");

fn main() {
    let matches = App::new("runar")
        .version(VERSION)
        .author(AUTHORS)
        .about(DESCRIPTION)
        .arg(
            Arg::with_name("recursive")
                .help("recursively watch directories")
                .short("r")
                .long("recursive")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("verbose")
                .help("increases the level of verbosity")
                .short("v")
                .long("verbose")
                .multiple(false)
                .takes_value(false),
        )
        .arg(
            Arg::with_name("exit-on-error")
                .help("exits with the same status code as COMMAND")
                .short("e")
                .long("exit-on-error")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("exit")
                .help("exit when COMMAND returns zero")
                .short("x")
                .long("exit")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("command")
                .help("the COMMAND to execute")
                .required(true)
                .value_name("COMMAND")
                .index(1),
        )
        .arg(
            Arg::with_name("files")
                .help("the file(s) to watch")
                .required(true)
                .value_name("FILE")
                .index(2)
                .multiple(true)
                .takes_value(true),
        )
        .get_matches();

    let opt_verbose = matches.is_present("verbose");
    let opt_recursive = matches.is_present("recursive");
    let opt_x_on_err = matches.is_present("exit-on-error");
    let opt_exit = matches.is_present("exit");
    let opt_command = matches.value_of("command").unwrap();
    let opt_files: Arc<Vec<String>> = Arc::new(
        matches
            .values_of("files")
            .unwrap()
            .map(String::from)
            .collect(),
    );

    // mutex with our pid
    let pid_ref: Arc<Mutex<Option<i32>>> = Arc::new(Mutex::new(None));

    // spawn the inotify handling thread with a clone of the reference to pid
    spawn_inotify_thread(pid_ref.clone(), opt_verbose, opt_recursive, opt_files);

    loop {
        let mut command = Command::new("sh");
        command.args(&["-c", opt_command]);

        let mut child = command.spawn().expect("Could not execute command");

        // grab the mutex lock, set pid and then drop the lock to release it
        let mut pid = pid_ref.lock().unwrap();
        *pid = Some(child.id() as i32);
        drop(pid);

        // wait for child process to exit
        let exitstatus = child.wait().expect("could not wait");

        if opt_verbose {
            println!("<runar> child process exited with {}", exitstatus);
        }

        if opt_x_on_err && !exitstatus.success() {
            process::exit(exitstatus.code().unwrap());
        }

        if opt_exit && exitstatus.success() {
            process::exit(0);
        }

        // sleep here to avoid loop becoming incredibly spammy
        thread::sleep(Duration::from_millis(1000));
    }
}

fn spawn_inotify_thread(
    pid_ref: Arc<Mutex<Option<i32>>>,
    opt_verbose: bool,
    opt_recursive: bool,
    opt_files: Arc<Vec<String>>,
) {
    std::thread::spawn(move || {
        let mut inotify = Inotify::init().expect("Error while initializing inotify instance");

        for i in 0..opt_files.len() {
            // TODO recurse through dirs and add watches
            // TODO better handle error, what can go wrong? kernel inotify limit?
            inotify
                .add_watch(opt_files[i].clone(), WatchMask::MODIFY)
                .expect("Could not add watch");
        }

        loop {
            let mut buffer = [0; 1024]; // buffer to store inotify events

            inotify
                .read_events_blocking(&mut buffer)
                .expect("Error while reading events");

            if opt_verbose {
                // TODO list which files
                println!("<runar> files modified");
            }

            safe_kill(&pid_ref);

            // sleep and then clear the event queue by doing a non-blocking read
            // prevents us from restarting multiple times because of burst changes
            thread::sleep(Duration::from_millis(2000));
            inotify
                .read_events(&mut buffer)
                .expect("Could not read events");
        }
    });
}

fn safe_kill(pid_ref: &Arc<Mutex<Option<i32>>>) {
    let pid = pid_ref.lock().unwrap();

    match *pid {
        None => eprintln!("<runar> Error: no pid to kill!"),
        Some(pid) => match kill(Pid::from_raw(pid), SIGTERM) {
            Ok(_) => (),
            Err(e) => eprintln!("<runar> Kill got error: {}", e),
        },
    }
}
