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

// TODO
// notify library
// correct stdout/stderr
// handle ctrl-c
//   respond immediately
//   start cleaning
//   force exit after 10s
// handle other signals https://rust-cli.github.io/book/in-depth/signals.html

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
                .takes_value(false),
        )
        .arg(
            Arg::with_name("exit")
                .help("exit when COMMAND exits")
                .long("exit")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("exit-on-error")
                .help("exit when COMMAND returns non-zero")
                .long("exit-on-error")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("command")
                .help("sets the input file to use")
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

    let opt_command = matches.value_of("command").unwrap();
    let opt_verbose = matches.is_present("verbose");
    let opt_files = matches.values_of("files").unwrap();

    println!("hey cmd({}) verbose({}) files({:?})", opt_command, opt_verbose, opt_files);

    // mutex with our pid
    let pid_ref: Arc<Mutex<Option<i32>>> = Arc::new(Mutex::new(None));

    // spawn the inotify handling thread with a clone of the reference to pid
    spawn_inotify_thread(pid_ref.clone());

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
        println!("Child died, {}", exitstatus);

        // sleep here to avoid loop becoming incredibly spammy
        thread::sleep(Duration::from_millis(1000));
    }
}

fn spawn_inotify_thread(pid_ref: Arc<Mutex<Option<i32>>>) {
    std::thread::spawn(move || {
        let mut inotify = Inotify::init().expect("Error while initializing inotify instance");

        // TODO recurse through dirs and add watches
        inotify
            .add_watch(".", WatchMask::MODIFY)
            .expect("Could not add watch");

        loop {
            let mut buffer = [0; 1024]; // buffer to store inotify events
            inotify
                .read_events_blocking(&mut buffer)
                .expect("Error while reading events");

            let pid = pid_ref.lock().unwrap();
            match *pid {
                None => println!("no pid to kill!"),
                Some(pid) => safe_kill(pid),
            }

            // sleep and then clear the event queue by doing a non-blocking read
            // prevents us from restarting multiple times because of burst changes
            thread::sleep(Duration::from_millis(2000));
            inotify
                .read_events(&mut buffer)
                .expect("Could not read events");
        }
    });
}

fn safe_kill(pid: i32) {
    match kill(Pid::from_raw(pid), SIGTERM) {
        Ok(_) => (),
        Err(e) => println!("Kill got error: {}", e),
    }
}
