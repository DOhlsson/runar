mod event_handler;
mod parse_args;

use std::os::unix::process::CommandExt;
use std::process::{self, Command, ExitCode};

use nix::errno::Errno;
use nix::poll::PollTimeout;
use nix::sys::prctl;
use nix::sys::signal::{kill, Signal};
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::{self, Pid};

use event_handler::{Event, EventHandler};
use parse_args::{parse_args, Options};

#[derive(Clone, Copy, Debug)]
/// Child process state
enum ChildState {
    Alive,
    Dormant,
    Restarting,
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
            eprintln!("<runar> Error: {e}");
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

    // The exit status of previously run of the command
    let mut exitstatus = 0;

    let mut child_pid = spawn_child(opts);
    let mut event;
    let mut state = ChildState::Alive;

    loop {
        // TODO: the ultimate goal is to get rid of the need for tracking ChildState,
        //       Could be possible using pidfd
        //       Or by using kill with signal 0 to check process state
        event = match state {
            ChildState::Restarting => {
                // Wait 100ms and then clear inotify of any residual changes to files
                // We do not care about changed files here since we are restarting anyway
                let res = handler.wait_signals(PollTimeout::from(100_u16))?;
                handler.clear_inotify()?;
                res
            }
            ChildState::Alive | ChildState::Dormant => handler.wait(-1)?,
        };

        // TODO debug level
        if opts.verbose {
            println!("<runar> main loop state & event ({state:?}, {event:?})");
        }

        match (event, state) {
            (Event::Terminate, ChildState::Alive) => {
                term_wait_kill(child_pid, &mut handler, opts);
                break;
            }
            (Event::Terminate, ChildState::Restarting | ChildState::Dormant) => {
                break;
            }
            (Event::FilesChanged, ChildState::Alive) => {
                term_wait_kill(child_pid, &mut handler, opts);
                state = ChildState::Restarting; // Restart child
            }
            (Event::FilesChanged, ChildState::Dormant) => {
                state = ChildState::Restarting;
            }
            (Event::ChildExit(dead_pid), ChildState::Alive) if dead_pid == child_pid => {
                let child_status = waitpid(child_pid, Some(WaitPidFlag::WNOHANG))?;
                exitstatus = match child_status {
                    WaitStatus::Exited(_, status) => status as u8,
                    WaitStatus::Signaled(_, signal, _) => 128 + signal as u8,
                    status => {
                        eprintln!("<runar> Error: Unhandled status {status:?}");
                        continue;
                    }
                };

                // Kill all children in pgrp
                term_wait_kill(child_pid, &mut handler, opts);

                if opts.verbose {
                    println!("<runar> child process exited with {exitstatus}");
                }

                if opts.exit_on_zero && exitstatus == 0 {
                    break;
                }

                if opts.exit_on_error && exitstatus != 0 {
                    break;
                }

                if opts.restart_on_zero && exitstatus == 0 {
                    // Next loop we will wait a bit to prevent the child restart loop from spazzing out
                    state = ChildState::Restarting;
                } else if opts.restart_on_error && exitstatus != 0 {
                    state = ChildState::Restarting;
                } else {
                    state = ChildState::Dormant;
                }
            }
            (Event::ChildExit(dead_pid), _) => {
                // Some inherited child died, this cleans it up
                match waitpid(dead_pid, Some(WaitPidFlag::WNOHANG)) {
                    Ok(_) => (),
                    Err(Errno::ECHILD) => (), // Child was waited on before we handled the signal
                    Err(e) => return Err(e),
                };
            }
            (Event::Nothing, ChildState::Alive | ChildState::Dormant) => (),
            // restart process?
            (_, ChildState::Restarting) => {
                // should needs to know if we should restart or not
                // file change is always a restart condition,
                // exit status is not
                // should also take into account if the exit status was voluntary or not
                child_pid = spawn_child(opts);
                state = ChildState::Alive;
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

    let child_pid = command.spawn().expect("Could not execute command").id() as i32;

    if opts.verbose {
        println!("<runar> child process spawned with pid {child_pid}");
    }

    Pid::from_raw(child_pid)
}

// Kills all processes in the process group
fn term_wait_kill(pid: Pid, handler: &mut EventHandler, opts: &Options) {
    let pgrp = Pid::from_raw(-pid.as_raw());

    // Send terminate signal to children, giving them time to terminate before we kill everything
    match kill(pgrp, Signal::SIGTERM) {
        Ok(()) => (),
        Err(Errno::ESRCH) => return, // No processes left in group
        Err(e) => {
            eprintln!("<runar> Kill got error: {e}");
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
