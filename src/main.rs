use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use nix::sys::signal::{kill, SIGTERM};
use nix::unistd::Pid;

use inotify::{Inotify, WatchMask};

// TODO
// cli parsing
//   usage
//   help, -h
//   -r
//   -v
//   --exit-on-error
// notify library
// correct stdout/stderr
// handle ctrl-c
//   respond immediately
//   start cleaning
//   force exit after 10s
// tests

fn main() {
    // mutex with our pid
    let pid_ref: Arc<Mutex<Option<i32>>> = Arc::new(Mutex::new(None));

    // spawn the inotify handling thread with a clone of the reference to pid
    spawn_inotify_thread(pid_ref.clone());

    loop {
        let mut command = Command::new("bash");
        command.args(&["-c", "for i in {0..3}; do echo $i; sleep 1s; done"]);

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
            thread::sleep(Duration::from_millis(2000));
            inotify.read_events(&mut buffer).expect("Could not read events");
        }
    });
}

fn safe_kill(pid: i32) {
    match kill(Pid::from_raw(pid), SIGTERM) {
        Ok(_) => (),
        Err(e) => println!("Kill got error: {}", e),
    }
}
