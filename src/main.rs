use std::os::unix::process::CommandExt;
use std::process;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use nix::errno::Errno;
use nix::sys::signal::{kill, SIGKILL, SIGTERM};
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::Pid;

use inotify::{Inotify, WatchMask};

use clap::{App, Arg};

use walkdir::WalkDir;

use libc::{prctl, PR_SET_CHILD_SUBREAPER};

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

    unsafe {
        // Become a Sub Reaper, taking on the responsibiliy of orphaned processess
        prctl(PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0);
    }

    if opt_verbose {
        println!("<runar> started with pid {}", process::id());
    }

    // mutex with child process pid
    let pid_ref: Arc<Mutex<Option<i32>>> = Arc::new(Mutex::new(None));

    // spawn the inotify handling thread with a clone of the reference to pid
    spawn_inotify_thread(pid_ref.clone(), opt_verbose, opt_recursive, opt_files);

    loop {
        let mut command = Command::new("sh");
        command.args(&["-c", opt_command]);

        // the child process needs to set a process group so that we can kill it later
        // there is a groups() function in nightly that could be used once it's stabilized
        unsafe {
            command.pre_exec(|| {
                nix::unistd::setpgid(Pid::from_raw(0), Pid::from_raw(process::id() as i32))
                    .unwrap();
                return Ok(());
            });
        }

        let mut child = command.spawn().expect("Could not execute command");

        // grab the mutex lock, set pid and then drop the lock to release it
        let mut pid = pid_ref.lock().unwrap();
        *pid = Some(child.id() as i32);
        drop(pid);

        // wait for child process to exit
        let exitstatus = child.wait().expect("could not wait");

        // TODO waitpid pgrp here?

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
        thread::sleep(Duration::from_millis(1_000));
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
            let target = opt_files[i].clone();
            if opt_recursive {
                for entry in WalkDir::new(target).into_iter() {
                    let path = entry.unwrap().into_path();
                    inotify
                        .add_watch(path, WatchMask::MODIFY)
                        .expect("Could not add watch");
                }
            } else {
                inotify
                    .add_watch(target, WatchMask::MODIFY)
                    .expect("Could not add watch");
            }
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
            thread::sleep(Duration::from_millis(1_000));
            inotify
                .read_events(&mut buffer)
                .expect("Could not read events");
        }
    });
}

fn safe_kill(pid_ref: &Arc<Mutex<Option<i32>>>) {
    let pid = pid_ref.lock().unwrap();

    let pgrp = match *pid {
        None => {
            eprintln!("<runar> Error: no pid to kill!");
            return;
        }
        // The negative pid means that it is a pgrp instead
        Some(pid) => Pid::from_raw(-pid),
    };

    let kill_res = kill(pgrp, SIGTERM);

    if let Err(e) = kill_res {
        eprintln!("<runar> Kill got error: {}", e);
        return;
    }

    thread::sleep(Duration::from_millis(10_000)); // TODO make configurable

    // We want the first round of reaping to be non-blocking so that kill actually gets called
    let mut waitflag = Some(WaitPidFlag::WNOHANG);

    // We need to reap all the children
    loop {
        match waitpid(pgrp, waitflag) {
            Ok(WaitStatus::StillAlive) => {
                println!("<runar> Some children took too long to exit, will now get SIGKILLed");
                kill(pgrp, SIGKILL).unwrap();
            }
            Ok(_) => (),                                  // Reaped succesfully
            Err(nix::Error::Sys(Errno::ECHILD)) => break, // No more children to reap
            Err(e) => {
                eprintln!("<runar> Unexpected error while reaping {}", e);
                break;
            }
        }
        waitflag = None; // Unset waitflag so that next waitpid becomes blocking
    }
}
