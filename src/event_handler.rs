use std::os::fd::AsFd;
use std::{cmp, process};

use nix::errno::Errno;
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use nix::sys::epoll::Epoll;
use nix::sys::epoll::{EpollCreateFlags, EpollEvent, EpollFlags};
use nix::sys::inotify::{AddWatchFlags, InitFlags, Inotify};
use nix::sys::signal::Signal;
use nix::sys::signalfd::{SfdFlags, SignalFd};
use nix::unistd::Pid;

use walkdir::WalkDir;

use crate::parse_args::Options;

const SIGNAL_EVENT: u64 = 1;
const INOTIFY_EVENT: u64 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Event {
    Terminate,
    FilesChanged,
    ChildExit(Pid),
    Nothing,
}

pub struct EventHandler {
    epoll: Epoll,
    inotify: Inotify,
    signalfd: SignalFd,
}

impl EventHandler {
    pub fn create(opts: &Options) -> Result<EventHandler, Errno> {
        opts.sigmask.thread_block()?;

        let signalfd = SignalFd::with_flags(&opts.sigmask, SfdFlags::SFD_NONBLOCK)?;
        let inotify = setup_inotify(opts);

        let signal_ep_ev = EpollEvent::new(EpollFlags::EPOLLIN, SIGNAL_EVENT);
        let inotify_ep_ev = EpollEvent::new(EpollFlags::EPOLLIN, INOTIFY_EVENT);

        let epoll = Epoll::new(EpollCreateFlags::EPOLL_CLOEXEC)?;
        epoll.add(&signalfd, signal_ep_ev)?;
        epoll.add(&inotify, inotify_ep_ev)?;

        Ok(EventHandler {
            inotify,
            epoll,
            signalfd,
        })
    }

    pub fn wait_signals(&mut self, timeout: i32) -> Result<Event, Errno> {
        let mut pfd = [PollFd::new(self.signalfd.as_fd(), PollFlags::POLLIN)];

        let res = poll(&mut pfd, PollTimeout::from(timeout as u16))?;

        if res.is_positive() {
            Ok(read_signal(&self.signalfd))
        } else {
            Ok(Event::Nothing)
        }
    }

    pub fn wait(&mut self, timeout: i32) -> Result<Event, Errno> {
        let mut ep_evs = [EpollEvent::empty(); 10];

        let ready_fds = self.epoll.wait(&mut ep_evs, timeout as u16)?;

        let mut event = Event::Nothing;

        // TODO rewrite as iterator to pick highest
        for ev in &ep_evs[..ready_fds] {
            let data = ev.data();
            let new_event;
            if data == SIGNAL_EVENT {
                new_event = read_signal(&self.signalfd);
            } else if data == INOTIFY_EVENT {
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

fn read_signal(signalfd: &SignalFd) -> Event {
    match signalfd.read_signal() {
        Ok(Some(sig)) => {
            let signal = Signal::try_from(sig.ssi_signo as i32).unwrap();

            use nix::sys::signal::Signal::*;
            match signal {
                SIGTERM | SIGINT | SIGHUP => Event::Terminate,
                SIGCHLD => Event::ChildExit(Pid::from_raw(sig.ssi_pid as i32)),
                _ => {
                    eprintln!("<runar> Unexpected signal {} caught by signalfd", signal);
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
    }
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
