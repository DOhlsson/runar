mod parse_args;

use std::ffi::{OsStr, OsString};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use nix::errno::Errno;
use nix::sys::inotify::{AddWatchFlags, InitFlags, Inotify};
use nix::sys::prctl;
use nix::sys::signal::{kill, SIGKILL, SIGTERM};
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::Pid;

use walkdir::WalkDir;

use parse_args::parse_args;

fn main() {
    let opts = parse_args();
    if opts.is_none() {
        return;
    }

    let opts = opts.unwrap();

    if opts.verbose {
        println!("<runar> started with pid {}", process::id());
    }

    let inotify = setup_inotify(opts.files, opts.recursive);

    // mutex with child process pid
    let pid_ref: Arc<Mutex<Option<i32>>> = Arc::new(Mutex::new(None));

    // spawn the inotify handling thread with a clone of the reference to pid
    spawn_inotify_thread(pid_ref.clone(), inotify, opts.verbose, opts.kill_timer);

    // Become a subreaper, taking on the responsibiliy of handling orphaned processess
    prctl::set_child_subreaper(true).unwrap();

    loop {
        let mut command = Command::new("sh");
        command.args(&[OsStr::new("-c"), opts.command.as_os_str()]);

        // the child process needs to set a process group so that we can kill it later
        // there is a groups() function in nightly that could be used once it's stabilized
        unsafe {
            command.pre_exec(|| {
                nix::unistd::setpgid(Pid::from_raw(0), Pid::from_raw(process::id() as i32))
                    .unwrap();
                Ok(())
            });
        }

        // TODO Should not be allowed to spawn before inotify thread is done, use mutex for this?
        let mut child = command.spawn().expect("Could not execute command");
        let child_pid = child.id() as i32;
        let pgrp = Pid::from_raw(-child_pid);

        if opts.verbose {
            println!("<runar> child process spawned with pid {}", child_pid);
        }

        // grab the mutex lock, set pid and then drop the lock to release it
        let mut pid = pid_ref.lock().unwrap();
        *pid = Some(child_pid);
        drop(pid);

        // wait for child process to exit
        let exitstatus = child.wait().expect("could not wait");

        // We need to reap all the children
        loop {
            match waitpid(pgrp, None) {
                Ok(_) => (),                 // Reaped succesfully
                Err(Errno::ECHILD) => break, // No more children to reap
                Err(e) => {
                    eprintln!("<runar> Unexpected error while reaping {}", e);
                    break;
                }
            }
        }

        if opts.verbose {
            println!("<runar> child process exited with {}", exitstatus);
        }

        if opts.error && !exitstatus.success() {
            if let Some(code) = exitstatus.code() {
                process::exit(code);
            }
            if let Some(signal) = exitstatus.signal() {
                process::exit(signal);
            }
            process::exit(1);
        }

        if opts.exit && exitstatus.success() {
            process::exit(0);
        }

        // sleep here to avoid loop becoming incredibly spammy
        thread::sleep(Duration::from_millis(100));
    }
}

// Set up Inotify instance
fn setup_inotify(files: Vec<OsString>, opt_recursive: bool) -> Inotify {
    let inotify =
        Inotify::init(InitFlags::IN_CLOEXEC).expect("Error while initializing inotify instance");

    for file in files {
        if opt_recursive {
            // TODO could we do some iterator magic here?
            for entry in WalkDir::new(&file).into_iter() {
                match entry {
                    Err(e) => {
                        let e = e.io_error().unwrap();
                        if e.kind() == std::io::ErrorKind::NotFound {
                            eprintln!(
                                "<runar> No such file or directory: {}",
                                file.into_string().unwrap()
                            );
                            process::exit(1);
                        } else {
                            eprintln!("<runar> Unexpected walkdir error {}", e);
                        }
                    }
                    Ok(entry) => {
                        let path = entry.into_path();

                        // TODO generalize error handling for inotify
                        inotify
                            .add_watch(&path, AddWatchFlags::IN_MODIFY)
                            .expect("Could not add watch");
                    }
                };
            }
        } else {
            match inotify.add_watch(file.as_os_str(), AddWatchFlags::IN_MODIFY) {
                Ok(_) => (),
                Err(Errno::ENOENT) => {
                    eprintln!(
                        "<runar> No such file or directory: {}",
                        file.into_string().unwrap()
                    );
                    process::exit(1);
                }
                Err(e) => {
                    eprintln!("<runar> Unexpected inotify error {}", e);
                    process::exit(1);
                }
            }
        }
    }

    inotify
}

fn spawn_inotify_thread(
    pid_ref: Arc<Mutex<Option<i32>>>,
    inotify: Inotify,
    opt_verbose: bool,
    opt_kill_timer: u64,
) {
    std::thread::spawn(move || loop {
        let events = inotify.read_events().expect("Error while reading events");

        if opt_verbose {
            let names: String = events
                .into_iter()
                .filter(|ev| ev.name.is_some())
                .take(10)
                .map(|ev| ev.name.unwrap().into_string().unwrap())
                .collect();
            println!("<runar> files modified: {}", names);
        }

        safe_kill(&pid_ref, opt_verbose, opt_kill_timer);
    });
}

fn safe_kill(pid_ref: &Arc<Mutex<Option<i32>>>, opt_verbose: bool, opt_kill_timer: u64) {
    let pid = pid_ref.lock().unwrap();

    // TODO use pid for SIGTERM
    #[allow(unused_variables)]
    let (pid, pgrp) = match *pid {
        None => {
            eprintln!("<runar> Error: no pid to kill!");
            return;
        }
        // The negative pid means that it is a pgrp instead
        Some(pid) => (Pid::from_raw(pid), Pid::from_raw(-pid)),
    };

    // Send terminate signal to child, giving it time to terminate before we kill everything
    let kill_res = kill(pgrp, SIGTERM);

    if let Err(e) = kill_res {
        eprintln!("<runar> Kill got error: {}", e);
        return;
    }

    thread::sleep(Duration::from_millis(opt_kill_timer));

    // If any process in the process group is still alive, we kill the entire group
    // This is so that we clean up any orphaned grandchildren that are still alive
    if let Ok(WaitStatus::StillAlive) = waitpid(pgrp, Some(WaitPidFlag::WNOHANG)) {
        if opt_verbose {
            println!("<runar> Some children took too long to exit, will now get SIGKILLed");
        }
        kill(pgrp, SIGKILL).unwrap();
    }
}
