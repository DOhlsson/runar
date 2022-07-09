extern crate nix;

use std::env::Args;
use std::process::Command;
use std::thread;
use std::time::Duration;
use std::{env, process};

use nix::sys::signal::{signal, SigHandler, Signal};

fn main() {
    let mut args = env::args();
    let runartest = args.next().unwrap();
    let next = args.next();

    if next.is_none() {
        eprintln!("Usage: runartest <name> <mode>");
        process::exit(1);
    }

    let name = next.unwrap();

    println!("start {}", name);

    match args.next().as_deref() {
        Some("success") => {}
        Some("error") => {
            eprintln!("err {}", name);
            process::exit(13);
        }
        Some("sleep") => {
            thread::sleep(Duration::from_millis(1_000));
        }
        Some("run") => loop {
            thread::sleep(Duration::from_millis(100));
            println!("a");
        },
        Some("hang") => {
            unsafe {
                signal(Signal::SIGINT, SigHandler::SigIgn).unwrap();
                signal(Signal::SIGTERM, SigHandler::SigIgn).unwrap();
            }
            thread::sleep(Duration::from_millis(1_000));
        }
        Some("child") => {
            // spawns a child runartest and exits, making the child orphaned
            spawn_child(runartest, args, false);
        }
        Some("waitchild") => {
            // spawns a child runartest and waits for it
            spawn_child(runartest, args, true);
        }
        Some(_) => {
            eprintln!("Unknown argument");
            process::exit(1);
        }
        None => {
            eprintln!("Expects argument");
            process::exit(1);
        }
    }

    println!("end {}", name);
}

fn spawn_child(runartest: String, args: Args, wait: bool) {
    let mut command = Command::new(runartest);
    let childargs: Vec<String> = args.collect();

    match childargs.len() {
        0 => {
            eprintln!("Child subcommand expects argument");
            process::exit(1);
        }
        _ => {
            command.args(childargs);
            let mut child = command.spawn().expect("Could not create child");
            if wait {
                child.wait().unwrap();
            }
        }
    }
}
