extern crate assert_cmd;

// TODO use test script instead

mod integration {
    use std::time::Duration;
    use std::fs::File;
    use std::io::prelude::*;
    use std::thread;

    use assert_cmd::Command;

    #[test]
    fn without_args_fails() {
        let assert = Command::cargo_bin("runar").unwrap().assert();
        assert.failure();
    }

    #[test]
    fn with_only_cmd_arg_fails() {
        let assert = Command::cargo_bin("runar").unwrap().args(["foo"]).assert();
        assert.failure();
    }

    #[test]
    fn exit_flag_with_success() {
        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-x", "sleep 0.1s", "."])
            .timeout(Duration::from_millis(200))
            .assert();
        assert.success();
    }

    #[test]
    fn exit_flag_with_error() {
        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-x", "sleep 0.1s; exit 1", "."])
            .timeout(Duration::from_millis(200))
            .assert();
        assert.interrupted();
    }

    #[test]
    fn exit_on_error_flag_with_success() {
        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-e", "sleep 0.1s", "."])
            .timeout(Duration::from_millis(200))
            .assert();
        assert.interrupted();
    }

    #[test]
    fn exit_on_error_flag_with_error() {
        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-e", "sleep 0.1s; exit 13", "."])
            .timeout(Duration::from_millis(200))
            .assert();
        assert.code(13);
    }

    #[test]
    fn restart_on_error() {
        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["echo start; sleep 0.07s; exit 1", "."])
            .timeout(Duration::from_millis(200))
            .assert();
        assert.stdout("start\nstart\n");
    }

    #[test]
    fn file_watch() {
        thread::spawn(|| {
            thread::sleep(Duration::from_millis(50));
            let mut file = File::create("./tests/data/file1").unwrap();
            file.write_all(&[0u8; 0]).unwrap();
            file.flush().unwrap();
        });

        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["echo start; sleep 1s; echo end", "tests/data/file1"])
            .timeout(Duration::from_millis(200))
            .assert();
        assert.stdout("start\nstart\nend\n");
    }

    #[test]
    fn recursive_file_watch() {
        thread::spawn(|| {
            thread::sleep(Duration::from_millis(50));
            let mut file = File::create("./tests/data/dir/file2").unwrap();
            file.write_all(&[0u8; 0]).unwrap();
            file.flush().unwrap();
        });

        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["echo start; sleep 1s; echo end", "tests/data/file1"])
            .timeout(Duration::from_millis(200))
            .assert();
        assert.stdout("start\nstart\nend\n");
    }

    #[test]
    fn file_not_found() {
        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["echo start; sleep 1s; echo end", "tests/data/does_not_exist"])
            .timeout(Duration::from_millis(200))
            .assert();
        assert.code(1);
    }

    #[test]
    fn recursive_file_not_found() {
        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-r", "echo start; sleep 1s; echo end", "tests/data/does_not_exist"])
            .timeout(Duration::from_millis(200))
            .assert();
        assert.code(1);
    }

    #[test]
    fn uninterruptible_cmd() {
        thread::spawn(|| {
            thread::sleep(Duration::from_millis(50));
            let mut file = File::create("./tests/data/file1").unwrap();
            file.write_all(&[0u8; 0]).unwrap();
            file.flush().unwrap();
        });

        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-k", "10", "./utility/script.sh -t -s 1", "tests/data/file1"])
            .timeout(Duration::from_millis(200))
            .assert();
        assert.stdout("start\nstart\nend\n");
    }
}
