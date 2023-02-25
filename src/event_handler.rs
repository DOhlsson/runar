use std::os::unix::prelude::AsRawFd;
use std::{cmp, process};

use nix::errno::Errno;
use nix::sys::epoll;
use nix::sys::epoll::{EpollCreateFlags, EpollEvent, EpollFlags, EpollOp};
use nix::sys::inotify::{AddWatchFlags, InitFlags, Inotify};
use nix::sys::signal::Signal;
use nix::sys::signalfd::{SfdFlags, SignalFd};
use nix::unistd::Pid;

use walkdir::WalkDir;

use crate::parse_args::Options;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Event {
    Terminate,
    FilesChanged,
    ChildExit(Pid),
    Nothing,
}

pub struct EventHandler {
    raw_epoll_fd: i32,
    inotify: Inotify,
    raw_inotify_fd: i32,
    raw_signal_fd: i32,
    signalfd: SignalFd,
}

impl EventHandler {
    pub fn create(opts: &Options) -> Result<EventHandler, Errno> {
        opts.sigmask.thread_block()?;

        let signalfd = SignalFd::with_flags(&opts.sigmask, SfdFlags::SFD_NONBLOCK)?;
        let raw_signal_fd = signalfd.as_raw_fd();

        let inotify = setup_inotify(opts);
        let raw_inotify_fd = inotify.as_raw_fd();

        let raw_epoll_fd = epoll::epoll_create1(EpollCreateFlags::EPOLL_CLOEXEC)?;
        let mut signal_ep_ev = EpollEvent::new(EpollFlags::EPOLLIN, raw_signal_fd as u64);
        let mut inotify_ep_ev = EpollEvent::new(EpollFlags::EPOLLIN, raw_inotify_fd as u64);

        epoll::epoll_ctl(
            raw_epoll_fd,
            EpollOp::EpollCtlAdd,
            raw_signal_fd,
            &mut signal_ep_ev,
        )?;

        epoll::epoll_ctl(
            raw_epoll_fd,
            EpollOp::EpollCtlAdd,
            raw_inotify_fd,
            &mut inotify_ep_ev,
        )?;

        Ok(EventHandler {
            raw_epoll_fd,
            inotify,
            raw_inotify_fd,
            raw_signal_fd,
            signalfd,
        })
    }

    pub fn wait_signals(&mut self, timeout: i32) -> Result<Event, Errno> {
        // Disable inotify in epoll_fd
        let mut inotify_ep_ev = EpollEvent::new(EpollFlags::empty(), self.raw_inotify_fd as u64);
        epoll::epoll_ctl(
            self.raw_epoll_fd,
            EpollOp::EpollCtlMod,
            self.raw_inotify_fd,
            &mut inotify_ep_ev,
        )?;

        let res = self.wait(timeout);

        // Re-enable inotify in epoll_fd
        let mut inotify_ep_ev = EpollEvent::new(EpollFlags::EPOLLIN, self.raw_inotify_fd as u64);
        epoll::epoll_ctl(
            self.raw_epoll_fd,
            EpollOp::EpollCtlMod,
            self.raw_inotify_fd,
            &mut inotify_ep_ev,
        )?;

        res
    }

    pub fn wait(&mut self, timeout: i32) -> Result<Event, Errno> {
        let mut ep_evs = [EpollEvent::empty(); 10];

        let ready_fds = epoll::epoll_wait(self.raw_epoll_fd, &mut ep_evs, timeout as isize)?;

        let mut event = Event::Nothing;

        // TODO rewrite as iterator to pick highest
        for ev in &ep_evs[..ready_fds] {
            let data = ev.data();
            let new_event;
            if data == self.raw_signal_fd as u64 {
                new_event = match self.signalfd.read_signal() {
                    Ok(Some(sig)) => {
                        let signal = Signal::try_from(sig.ssi_signo as i32).unwrap();

                        use nix::sys::signal::Signal::*;
                        match signal {
                            SIGTERM | SIGINT | SIGHUP => Event::Terminate,
                            SIGCHLD => Event::ChildExit(Pid::from_raw(sig.ssi_pid as i32)),
                            _ => {
                                eprintln!(
                                    "<runar> Unexpected signal {} caught by signalfd",
                                    signal
                                );
                                Event::Terminate
                            }
                        }
                    }
                    // there were no signals waiting (only happens when the SFD_NONBLOCK flag is set,
                    // otherwise the read_signal call blocks)
                    Ok(None) => Event::Nothing,
                    Err(e) => {
                        eprintln!("<runar> Error: {}", e);
                        Event::Terminate
                    }
                };
            } else if data == self.raw_inotify_fd as u64 {
                self.clear_inotify()?;

                // TODO write which files changed if verbose
                new_event = Event::FilesChanged;
            } else {
                eprintln!("<runar> epoll_wait returned unknown data");
                // TODO return error
                new_event = Event::Terminate;
            }

            // We will return the highest priority event
            event = cmp::min(event, new_event);
        }

        Ok(event)
    }

    pub fn clear_inotify(&mut self) -> Result<(), Errno> {
        match self.inotify.read_events() {
            Ok(_) => Ok(()),
            Err(Errno::EAGAIN) => Ok(()), // No events pending
            Err(e) => Err(e),
        }
    }

    // TODO close and drop functions
}

// Set up Inotify instance
// TODO clean up error handling here
fn setup_inotify(opts: &Options) -> Inotify {
    let inotify = Inotify::init(InitFlags::IN_CLOEXEC | InitFlags::IN_NONBLOCK)
        .expect("Error while initializing inotify instance");

    for file in &opts.files {
        if opts.recursive {
            // TODO could we do some iterator magic here?
            for entry in WalkDir::new(&file).into_iter() {
                match entry {
                    Err(e) => {
                        let e = e.io_error().unwrap();
                        if e.kind() == std::io::ErrorKind::NotFound {
                            eprintln!(
                                "<runar> No such file or directory: {}",
                                file.clone().into_string().unwrap()
                            );
                            process::exit(1);
                        } else {
                            eprintln!("<runar> Unexpected walkdir error {}", e);
                            process::exit(1);
                        }
                    }
                    Ok(entry) => {
                        let path = entry.into_path();

                        // TODO generalize error handling for inotify
                        inotify
                            .add_watch(&path, AddWatchFlags::IN_CLOSE_WRITE)
                            .expect("Could not add watch");
                    }
                };
            }
        } else {
            match inotify.add_watch(file.as_os_str(), AddWatchFlags::IN_CLOSE_WRITE) {
                Ok(_) => (),
                Err(Errno::ENOENT) => {
                    eprintln!(
                        "<runar> No such file or directory: {}",
                        file.clone().into_string().unwrap()
                    );
                    process::exit(1); // TODO handle error
                }
                Err(e) => {
                    eprintln!("<runar> Unexpected inotify error {}", e);
                    process::exit(1); // TODO handle error
                }
            }
        }
    }

    inotify
}
