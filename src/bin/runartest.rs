extern crate nix;

use nix::sys::signal::{signal, SigHandler, Signal};
use std::thread;
use std::time::Duration;
use std::{env, process};

fn main() {
    println!("start");

    /*
    let args: Vec<String> = env::args().collect();
    println!("args {:?}", args);
    // */

    let arg = env::args().nth(1);

    if arg.is_none() {
        println!("Expects argument");
        process::exit(1);
    }

    match arg.unwrap().as_str() {
        "success" => {}
        "error" => {
            process::exit(13);
        }
        "sleep" => {
            thread::sleep(Duration::from_millis(1000));
        }
        "run" => loop {
            thread::sleep(Duration::from_millis(100));
            println!("a");
        },
        "hang" => {
            unsafe {
                signal(Signal::SIGINT, SigHandler::SigIgn).unwrap();
                signal(Signal::SIGTERM, SigHandler::SigIgn).unwrap();
            }
            thread::sleep(Duration::from_millis(10_000));
        }
        _ => {
            println!("Unknown argument");
            process::exit(1);
        }
    }

    println!("end");
}
