use std::io::{self, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use shared_child::SharedChild;

struct ChildContainer {
    shared_child: SharedChild,
    child_created: Condvar,
}

fn main() {
    println!("Hello, world!");

    let mut command = Command::new("bash");
    command.args(&["-c", "for i in {0..3}; do echo $i; sleep 1s; done"]);
    //command.stdout(Stdio::piped());

    //test1(&mut command);

    let shared_child = SharedChild::spawn(&mut command).unwrap();
    let child_container = ChildContainer { shared_child };

    let container_arc = Arc::new(Mutex::new(child_container));
    let container_arc_clone = container_arc.clone();

    let notify_thread = std::thread::spawn(move || {
        for i in 0..5 {
            println!("thread {}", i);
            thread::sleep(Duration::from_millis(500));
        }

        println!("time to kill");
        container_arc_clone.lock().unwrap().shared_child.kill().unwrap();
        println!("after kill");
    });

    let exitstatus = container_arc.lock().unwrap().shared_child.wait().expect("command wasn't running");
    println!("foo3");

    println!("exit status: {}", exitstatus);

    thread::sleep(Duration::from_millis(1000));
}

// not worky
fn test1(command: &mut Command) {
    println!("foo1");

    let mut child = command.spawn().unwrap();
    println!("foo2");

    let child_ref = Arc::new(Mutex::new(child));

    let clone_ref = Arc::clone(&child_ref);

    thread::spawn(move || {
        for i in 0..5 {
            println!("thread {}", i);
            thread::sleep(Duration::from_millis(500));
        }

        //let child = child_ref.unwrap();
        (*clone_ref)
            .lock()
            .unwrap()
            .kill()
            .expect("Error killing child");
    });

    let exitstatus = (*child_ref)
        .lock()
        .unwrap()
        .wait()
        .expect("command wasn't running");
    println!("foo3");

    println!("exit status: {}", exitstatus);
}
