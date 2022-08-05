extern crate assert_cmd;
extern crate assert_fs;
extern crate test_binary;

// TODO add exitstatus to all tests

mod integration {
    use std::process::Child;
    use std::process::Stdio;
    use std::sync::Once;
    use std::thread;
    use std::time::Duration;

    use assert_cmd::assert::Assert;
    use assert_cmd::cargo::cargo_bin;
    use assert_cmd::Command;

    use assert_fs::fixture::ChildPath;
    use assert_fs::prelude::*;
    use assert_fs::TempDir;

    use nix::sys::signal::kill;
    use nix::sys::signal::Signal;
    use nix::unistd::Pid;

    use test_binary::build_mock_binary_with_opts;

    const TEST_BINARY_PATH: &str = env!("CARGO_BIN_EXE_runartest");
    static INIT: Once = Once::new();

    fn testprog() -> &'static str {
        INIT.call_once(|| {
            build_mock_binary_with_opts("runartest", None, vec!["runartest"]).unwrap();
        });

        TEST_BINARY_PATH
        /*
        let mut testprog = TEST_BINARY_PATH.to_owned();
        testprog.push_str(" ");
        testprog.push_str(arg);
        return testprog;
        */
    }

    fn run_runar(args: Vec<&str>) -> Child {
        std::process::Command::new(cargo_bin(env!("CARGO_PKG_NAME")))
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    }

    fn delayed_write_file(millis: u64, tmp_file: ChildPath) {
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(millis));
            tmp_file.write_str("my file").unwrap();
        });
    }

    fn delayed_sigterm(millis: u64, pid: i32) {
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(millis));
            kill(Pid::from_raw(pid), Signal::SIGTERM).unwrap();
        });
    }

    #[test]
    fn without_args_fails() {
        let assert = Command::cargo_bin("runar").unwrap().assert();

        // Too few arguments
        assert.failure();
    }

    #[test]
    fn with_only_cmd_arg_fails() {
        let assert = Command::cargo_bin("runar").unwrap().args(["foo"]).assert();

        // Too few arguments
        assert.failure();
    }

    #[test]
    fn file_not_found() {
        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-f", "./does_not_exist", "--", testprog(), "foo", "success"])
            .timeout(Duration::from_millis(200))
            .assert();

        // file not found
        assert.failure();

        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args([
                "-f",
                ".",
                "-f",
                "./does_not_exist",
                "--",
                testprog(),
                "foo",
                "success",
            ])
            .timeout(Duration::from_millis(200))
            .assert();

        // file not found
        assert.failure();
    }

    #[test]
    fn recursive_file_not_found() {
        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args([
                "-r",
                "-f",
                "./does_not_exist",
                "--",
                testprog(),
                "foo",
                "success",
            ])
            .timeout(Duration::from_millis(200))
            .assert();

        // file not found
        assert.failure();
    }

    #[test]
    fn exit_flag_with_success() {
        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-x", "--", testprog(), "foo", "success"])
            .timeout(Duration::from_millis(200))
            .assert();

        // runar starts runartest
        // runartest exits cleanly
        // runar exits cleanly
        assert.stdout("start foo\nend foo\n").stderr("").success();
    }

    #[test]
    fn exit_flag_with_error() {
        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-x", "--", testprog(), "foo", "error"])
            .timeout(Duration::from_millis(200))
            .assert();

        // runar starts runartest
        // runartest errors
        // runar restarts runartest
        // runartest errors
        // runar gets interrupted
        assert
            .stdout("start foo\nstart foo\n")
            .stderr("err foo\nerr foo\n")
            .interrupted();
    }

    #[test]
    fn exit_on_error_flag_with_success() {
        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-e", "--", testprog(), "foo", "success"])
            .timeout(Duration::from_millis(200))
            .assert();

        // runar starts runartest
        // runartest exits cleanly
        // runar restart runartest
        // runartest exits cleanly
        // runar gets interrupted
        assert
            .stdout("start foo\nend foo\nstart foo\nend foo\n")
            .stderr("")
            .interrupted();
    }

    #[test]
    fn exit_on_error_flag_with_error() {
        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-e", "--", testprog(), "foo", "error"])
            .timeout(Duration::from_millis(200))
            .assert();

        // runar starts runartest
        // runartest exits with status 13
        // runar exits with runartests status 13
        assert.stdout("start foo\n").stderr("err foo\n").code(13);
    }

    #[test]
    fn restart_on_error() {
        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["--", testprog(), "foo", "error"])
            .timeout(Duration::from_millis(200))
            .assert();

        // runar starts runartest
        // runartest errors
        // runar restarts runartest
        // runartest errors
        // runar gets interrupted
        assert
            .stdout("start foo\nstart foo\n")
            .stderr("err foo\nerr foo\n")
            .interrupted();
    }

    #[test]
    fn file_watch() {
        let tmp_dir = TempDir::new().unwrap();
        let tmp_file = tmp_dir.child("file");
        tmp_file.touch().unwrap();
        let file = tmp_file.to_str().unwrap();

        let runar = run_runar(vec!["-f", file, "--", testprog(), "foo", "sleep"]);

        delayed_write_file(200, tmp_file);
        delayed_sigterm(500, runar.id() as i32);

        let output = runar.wait_with_output().unwrap();
        let assert = Assert::new(output);

        // runar starts runartest
        // runartest sleeps
        // file is written
        // runar restarts runartest
        // runartest sleeps
        // runar gets sigterm
        // runar sends sigterm to runartest
        assert.stdout("start foo\nstart foo\n").stderr("");
    }

    #[test]
    fn recursive_file_watch() {
        let tmp_dir = TempDir::new().unwrap();
        let dir = tmp_dir.to_str().unwrap();
        let tmp_file = tmp_dir.child("deep/file");
        tmp_file.touch().unwrap();

        let runar = run_runar(vec!["-rf", dir, "--", testprog(), "foo", "sleep"]);

        delayed_write_file(200, tmp_file);
        delayed_sigterm(500, runar.id() as i32);

        let output = runar.wait_with_output().unwrap();
        let assert = Assert::new(output);

        // runar starts runartest
        // runartest sleeps
        // file is written
        // runar restarts runartest
        // runartest sleeps
        // runar gets sigterm
        // runar sends sigterm to runartest
        assert.stdout("start foo\nstart foo\n").stderr("");
    }

    #[test]
    fn uninterruptible_cmd() {
        let tmp_dir = TempDir::new().unwrap();
        let tmp_file = tmp_dir.child("file");
        tmp_file.touch().unwrap();
        let file = tmp_file.to_str().unwrap();

        let runar = run_runar(vec!["-xk10", "-f", file, "--", testprog(), "foo", "hang"]);

        delayed_write_file(200, tmp_file);
        delayed_sigterm(500, runar.id() as i32);

        let output = runar.wait_with_output().unwrap();
        let assert = Assert::new(output);

        // runar starts runartest
        // runartest hangs
        // file is written
        // runar attempts to restart runartest
        // runartest gets sigterm
        // runartest gets sigkill
        // runar restarts runartest
        // runar gets sigterm
        // runar sends sigterm and then sigkill to runartest
        assert.stdout("start foo\nstart foo\n").stderr("");
    }

    #[test]
    fn multiple_writes() {
        let tmp_dir = TempDir::new().unwrap();
        let dir = tmp_dir.to_str().unwrap();

        let tmp_file_1 = tmp_dir.child("file1");
        tmp_file_1.touch().unwrap();

        let tmp_file_2 = tmp_dir.child("file2");
        tmp_file_2.touch().unwrap();

        let tmp_file_3 = tmp_dir.child("file3");
        tmp_file_3.touch().unwrap();

        let runar = run_runar(vec!["-rf", dir, "--", testprog(), "foo", "sleep"]);

        delayed_write_file(200, tmp_file_1);
        delayed_write_file(230, tmp_file_2);
        delayed_write_file(260, tmp_file_3);
        delayed_sigterm(500, runar.id() as i32);

        let output = runar.wait_with_output().unwrap();
        let assert = Assert::new(output);

        // runar starts runartest
        // runartest sleeps
        // file1 is written
        // runar stops runartest
        // file2 and file3 are written
        // runar clears filewrites from buffer
        // runar starts runartest
        assert.stdout("start foo\nstart foo\n").stderr("");
    }

    #[test]
    fn file_watch_with_child_sleep() {
        let tmp_dir = TempDir::new().unwrap();
        let tmp_file = tmp_dir.child("file");
        tmp_file.touch().unwrap();
        let file = tmp_file.to_str().unwrap();

        let runar = run_runar(vec![
            "-k10",
            "-f",
            file,
            "--",
            testprog(),
            "foo",
            "waitchild",
            "bar",
            "sleep",
        ]);

        delayed_write_file(200, tmp_file);
        delayed_sigterm(500, runar.id() as i32);

        let output = runar.wait_with_output().unwrap();
        let assert = Assert::new(output);

        // runar starts runartest foo
        // foo starts child bar
        // foo waits on bar
        // bar sleeps
        // file is written
        // runar stops foo
        // runar cleans up bar
        // runar restarts foo
        assert
            .stdout("start foo\nstart bar\nstart foo\nstart bar\n")
            .stderr("");
    }

    #[test]
    fn file_watch_with_child_hang() {
        let tmp_dir = TempDir::new().unwrap();
        let tmp_file = tmp_dir.child("file");
        tmp_file.touch().unwrap();
        let file = tmp_file.to_str().unwrap();

        let runar = run_runar(vec![
            "-k10",
            "-f",
            file,
            "--",
            testprog(),
            "foo",
            "waitchild",
            "bar",
            "hang",
        ]);

        delayed_write_file(200, tmp_file);
        delayed_sigterm(500, runar.id() as i32);

        let output = runar.wait_with_output().unwrap();
        let assert = Assert::new(output);

        // runar starts runartest foo
        // foo starts child bar
        // foo waits on bar
        // bar hangs
        // file is written
        // runar stops foo
        // runar cleans up bar (by killing)
        // runar restarts foo
        assert
            .stdout("start foo\nstart bar\nstart foo\nstart bar\n")
            .stderr("");
    }

    #[test]
    fn grandchild_cleanup() {
        let runar = run_runar(vec!["-x", "--", testprog(), "foo", "child", "bar", "sleep"]);

        delayed_sigterm(500, runar.id() as i32);

        let output = runar.wait_with_output().unwrap();
        let assert = Assert::new(output);

        // runar starts runartest foo
        // foo starts child bar
        // foo exits
        // bar sleeps
        // runar attempts to exit
        // runar cleans up bar
        assert.stdout("start foo\nstart bar\nend foo\n").stderr("");
    }
}
