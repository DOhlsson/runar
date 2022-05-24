extern crate assert_cmd;
extern crate test_binary;

// TODO mock test data on the fly, possibly with TestDir

mod integration {
    use std::fs::File;
    use std::io::prelude::*;
    use std::sync::Once;
    use std::thread;
    use std::time::Duration;

    use assert_cmd::Command;
    use test_binary::build_mock_binary_with_opts;

    const TEST_BINARY_PATH: &str = env!("CARGO_BIN_EXE_runartest");
    static INIT: Once = Once::new();

    fn testprog(arg: &str) -> String {
        INIT.call_once(|| {
            build_mock_binary_with_opts("runartest", None, vec!["runartest"]).unwrap();
        });

        let mut testprog = TEST_BINARY_PATH.to_owned();
        testprog.push_str(" ");
        testprog.push_str(arg);
        return testprog;
    }

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
            .args(["-x", &testprog("success"), "."])
            .timeout(Duration::from_millis(200))
            .assert();

        assert.success().stdout("start\nend\n");
    }

    #[test]
    fn exit_flag_with_error() {
        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-x", &testprog("error"), "."])
            .timeout(Duration::from_millis(200))
            .assert();

        assert.interrupted();
    }

    #[test]
    fn exit_on_error_flag_with_success() {
        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-e", &testprog("success"), "."])
            .timeout(Duration::from_millis(200))
            .assert();

        assert.interrupted();
    }

    #[test]
    fn exit_on_error_flag_with_error() {
        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-e", &testprog("error"), "."])
            .timeout(Duration::from_millis(200))
            .assert();

        assert.code(13);
    }

    #[test]
    fn restart_on_error() {
        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args([&testprog("error"), "."])
            .timeout(Duration::from_millis(200))
            .assert();

        assert.stdout("start\nstart\n").interrupted();
    }

    #[test]
    fn file_watch() {
        thread::spawn(|| {
            thread::sleep(Duration::from_millis(200));
            let mut file = File::create("./tests/data/dir/file2").unwrap();
            file.write_all(&[0u8; 0]).unwrap();
            file.flush().unwrap();
        });

        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args([&testprog("sleep"), "tests/data/file1"])
            .timeout(Duration::from_millis(500))
            .assert();

        assert.stdout("start\nstart\nend\n");
    }

    #[test]
    fn recursive_file_watch() {
        thread::spawn(|| {
            thread::sleep(Duration::from_millis(200));
            let mut file = File::create("./tests/data/dir/file2").unwrap();
            file.write_all(&[0u8; 0]).unwrap();
            file.flush().unwrap();
        });

        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-r", &testprog("sleep"), "tests/data/"])
            .timeout(Duration::from_millis(500))
            .assert();

        assert.stdout("start\nstart\nend\n");
    }

    #[test]
    fn file_not_found() {
        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args([&testprog("success"), "tests/data/does_not_exist"])
            .timeout(Duration::from_millis(200))
            .assert();

        assert.code(1);
    }

    #[test]
    fn recursive_file_not_found() {
        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-r", &testprog("success"), "tests/data/does_not_exist"])
            .timeout(Duration::from_millis(200))
            .assert();

        assert.code(1);
    }

    #[test]
    fn uninterruptible_cmd() {
        thread::spawn(|| {
            thread::sleep(Duration::from_millis(200));
            let mut file = File::create("./tests/data/file1").unwrap();
            file.write_all(&[0u8; 0]).unwrap();
            file.flush().unwrap();
        });

        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-x", "-k", "10", &testprog("hang"), "tests/data/file1"])
            .timeout(Duration::from_millis(500))
            .assert();

        // end is actually incorrect here, but runar does not properly handle SIGTERM (yet)
        assert.stdout("start\nstart\nend\n");
    }

    #[test]
    fn multiple_writes() {
        thread::spawn(|| {
            thread::sleep(Duration::from_millis(200));
            let mut file = File::create("./tests/data/file1").unwrap();
            file.write_all(&[65, 10]).unwrap();
            file.flush().unwrap();

            thread::sleep(Duration::from_millis(100));
            let mut file2 = File::create("./tests/data/file2").unwrap();
            file2.write_all(&[66, 10]).unwrap();
            file2.flush().unwrap();
        });

        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-r", &testprog("sleep"), "tests/data/"])
            .timeout(Duration::from_millis(500))
            .assert();

        assert.stdout("start\nstart\nend\n");
    }
}
