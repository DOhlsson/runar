mod event_handler;
mod parse_args;

use std::os::unix::process::CommandExt;
use std::process;
use std::process::Command;
use std::process::ExitCode;

use nix::errno::Errno;
use nix::sys::prctl;
use nix::sys::signal::{kill, Signal};
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd;
use nix::unistd::Pid;

use event_handler::EventHandler;
use parse_args::{parse_args, Options};

use crate::event_handler::Event;

#[derive(Clone, Copy, Debug)]
enum State {
    Running,
    Dead,
}

fn main() -> ExitCode {
    let opts = match parse_args() {
        Err(code) => return code,
        Ok(res) => res,
    };

    // TODO debug level
    if opts.verbose {
        println!("<runar> started with pid {}", process::id());
    }

    let exitstatus = match run_loop(&opts) {
        Err(e) => {
            eprintln!("<runar> Error: {}", e);
            1
        }
        Ok(exitstatus) => exitstatus,
    };

    ExitCode::from(exitstatus)
}

fn run_loop(opts: &Options) -> Result<u8, Errno> {
    // Become a subreaper, taking on the responsibiliy of handling orphaned processess
    prctl::set_child_subreaper(true)?;

    let mut handler = EventHandler::create(opts)?;

    let mut exitstatus = 0;
    let mut child_pid = spawn_child(opts);
    let mut event;
    let mut state = State::Running;

    loop {
        event = match state {
            State::Dead => {
                let res = handler.wait_signals(100)?;
                handler.clear_inotify()?;
                res
            }
            State::Running => handler.wait(-1)?,
        };

        // TODO debug level
        if opts.verbose {
            println!("<runar> main loop state & event ({:?}, {:?})", state, event);
        }

        match (state, event) {
            (State::Running, Event::Terminate) => {
                term_wait_kill(child_pid, &mut handler, opts);
                break;
            }
            (State::Running, Event::FilesChanged) => {
                term_wait_kill(child_pid, &mut handler, opts);
                state = State::Dead; // Restart child
            }
            (State::Running, Event::ChildExit(dead_pid)) if dead_pid == child_pid => {
                let child_status = waitpid(child_pid, Some(WaitPidFlag::WNOHANG))?;
                let status = match child_status {
                    WaitStatus::Exited(_, status) => status as u8,
                    WaitStatus::Signaled(_, signal, _) => 128 + signal as u8,
                    status => {
                        eprintln!("<runar> Error: Unhandled status {:?}", status);
                        continue;
                    }
                };

                // Kill all children in pgrp
                term_wait_kill(child_pid, &mut handler, opts);

                if opts.verbose {
                    println!("<runar> child process exited with {}", status);
                }

                if opts.exit_on_zero && status == 0 {
                    break;
                }

                if opts.exit_on_error && status != 0 {
                    exitstatus = status;
                    break;
                }

                // Next loop we will wait a bit to prevent the child restart loop from spazzing out
                state = State::Dead;
            }
            (_, Event::ChildExit(dead_pid)) => {
                // Some inherited child died, this cleans it up
                match waitpid(dead_pid, Some(WaitPidFlag::WNOHANG)) {
                    Ok(_) => (),
                    Err(Errno::ECHILD) => (), // Child was cleaned up before we handled the signal
                    Err(e) => return Err(e),
                };
            }
            (State::Running, Event::Nothing) => (),
            (State::Dead, Event::Terminate) => {
                break;
            }
            // An unknown child exited
            (State::Dead, _) => {
                child_pid = spawn_child(opts);
                state = State::Running;
            }
        }
    }

    // TODO Always return same exitstatus as child

    Ok(exitstatus)
}

fn spawn_child(opts: &Options) -> Pid {
    let mut command = Command::new(&opts.command[0]);
    command.args(&opts.command[1..]);
    let sigmask = opts.sigmask;

    unsafe {
        command.pre_exec(move || {
            // the child inherits blocked signals, we must unblock them
            sigmask.thread_unblock().unwrap();

            // set a process group for the child, so we may easily kill the entire group
            // this group is inherited by all grandchildren (unless they change the group
            // themselves)
            // there is a process_group() function in rust 1.64 that could be used instead
            unistd::setpgid(Pid::from_raw(0), Pid::from_raw(process::id() as i32)).unwrap();

            Ok(())
        });
    }

    let child = command.spawn().expect("Could not execute command");
    let child_pid = child.id() as i32;

    if opts.verbose {
        println!("<runar> child process spawned with pid {}", child_pid);
    }

    Pid::from_raw(child_pid)
}

// Kills all processes in the process group
fn term_wait_kill(pid: Pid, handler: &mut EventHandler, opts: &Options) {
    let pgrp = Pid::from_raw(-pid.as_raw());

    // Send terminate signal to children, giving them time to terminate before we kill everything
    match kill(pgrp, Signal::SIGTERM) {
        Ok(_) => (),
        Err(Errno::ESRCH) => return, // No processes left in group
        Err(e) => {
            eprintln!("<runar> Kill got error: {}", e);
            return;
        }
    }

    handler.wait_signals(opts.kill_timer).unwrap();

    // If any process in the process group is still alive, we kill the entire group
    // This is so that we clean up any orphaned children that are still alive
    let mut kill_pgrp = false;
    while let Ok(wait_status) = waitpid(pgrp, Some(WaitPidFlag::WNOHANG)) {
        if wait_status == WaitStatus::StillAlive {
            kill_pgrp = true;
            break;
        }
    }

    if kill_pgrp {
        if opts.verbose {
            println!("<runar> Some children took too long to exit, will now get SIGKILLed");
        }
        kill(pgrp, Signal::SIGKILL).unwrap();
    }
}
