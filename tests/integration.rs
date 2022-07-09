extern crate assert_cmd;
extern crate assert_fs;
extern crate test_binary;

mod integration {
    use std::sync::Once;
    use std::thread;
    use std::time::Duration;

    use assert_cmd::Command;
    use assert_fs::fixture::ChildPath;
    use assert_fs::prelude::*;
    use assert_fs::TempDir;
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

    fn delayed_write_file(millis: u64, tmp_file: ChildPath) {
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(millis));
            tmp_file.write_str("my file").unwrap();
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
    fn exit_flag_with_success() {
        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-x", &testprog("foo success"), "."])
            .timeout(Duration::from_millis(200))
            .assert();

        // runar starts runartest
        // runartest exits cleanly
        // runar exits cleanly
        assert.stdout("start foo\nend foo\n").success();
    }

    #[test]
    fn exit_flag_with_error() {
        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-x", &testprog("foo error"), "."])
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
            .args(["-e", &testprog("foo success"), "."])
            .timeout(Duration::from_millis(200))
            .assert();

        // runar starts runartest
        // runartest exits cleanly
        // runar restart runartest
        // runartest exits cleanly
        // runar gets interrupted
        assert
            .stdout("start foo\nend foo\nstart foo\nend foo\n")
            .interrupted();
    }

    #[test]
    fn exit_on_error_flag_with_error() {
        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-e", &testprog("foo error"), "."])
            .timeout(Duration::from_millis(200))
            .assert();

        // runar starts runartest
        // runartest errors
        // runar exits with runartests error code
        assert.stdout("start foo\n").stderr("err foo\n").code(13);
    }

    #[test]
    fn restart_on_error() {
        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args([&testprog("foo error"), "."])
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

    #[cfg(failing_tests)]
    #[test]
    fn file_watch() {
        let tmp_dir = TempDir::new().unwrap();
        let tmp_file = tmp_dir.child("deep/file");
        tmp_file.touch().unwrap();
        let file = tmp_file.path().to_str().unwrap().to_owned();

        delayed_write_file(200, tmp_file);

        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args([&testprog("foo sleep"), &file])
            .timeout(Duration::from_millis(500))
            .assert();

        // runar starts runartest
        // runartest sleeps
        // file is written
        // runar restarts runartest
        // runartest sleeps
        // runartest gets interrupted
        // TODO broken because runar does not handle the interrupt properly
        assert.stdout("start foo\nstart foo\n").interrupted();
    }

    #[cfg(failing_tests)]
    #[test]
    fn recursive_file_watch() {
        let tmp_dir = TempDir::new().unwrap();
        let dir = tmp_dir.path().to_str().unwrap().to_owned();

        let tmp_file = tmp_dir.child("deep/file");
        tmp_file.touch().unwrap();

        delayed_write_file(200, tmp_file);

        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-r", &testprog("foo sleep"), &dir])
            .timeout(Duration::from_millis(500))
            .assert();

        // runar starts runartest
        // runartest sleeps
        // file is written
        // runar restarts runartest
        // runartest sleeps
        // runartest gets interrupted
        // TODO broken because runar does not handle the interrupt properly
        assert.stdout("start foo\nstart foo\n").interrupted();
    }

    #[test]
    fn file_not_found() {
        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args([&testprog("foo success"), "./does_not_exist"])
            .timeout(Duration::from_millis(200))
            .assert();

        assert.code(1);

        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args([&testprog("foo success"), ".", "./does_not_exist"])
            .timeout(Duration::from_millis(200))
            .assert();

        assert.code(1);
    }

    #[test]
    fn recursive_file_not_found() {
        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-r", &testprog("foo success"), "./does_not_exist"])
            .timeout(Duration::from_millis(200))
            .assert();

        assert.code(1);
    }

    #[cfg(failing_tests)]
    #[test]
    fn uninterruptible_cmd() {
        let tmp_dir = TempDir::new().unwrap();
        let tmp_file = tmp_dir.child("file");
        tmp_file.touch().unwrap();
        let file = tmp_file.path().to_str().unwrap().to_owned();

        delayed_write_file(200, tmp_file);

        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-x", "-k", "10", &testprog("foo hang"), &file])
            .timeout(Duration::from_millis(500))
            .assert();

        // runar starts runartest
        // runartest hangs
        // file is written
        // runar attempts to restart runartest
        // runartest gets SIGTERM'd
        // runartest gets SIGKILL'd
        // runar restarts runartest
        // TODO broken because runar does not handle the interrupt properly
        assert.stdout("start foo\nstart foo\n").interrupted();
    }

    #[cfg(failing_tests)]
    #[test]
    fn multiple_writes() {
        let tmp_dir = TempDir::new().unwrap();
        let dir = tmp_dir.path().to_str().unwrap().to_owned();

        let tmp_file_1 = tmp_dir.child("file1");
        tmp_file_1.touch().unwrap();

        let tmp_file_2 = tmp_dir.child("file2");
        tmp_file_2.touch().unwrap();

        delayed_write_file(200, tmp_file_1);
        delayed_write_file(250, tmp_file_2);

        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-r", &testprog("foo sleep"), &dir])
            .timeout(Duration::from_millis(500))
            .assert();

        // runar starts runartest
        // runartest sleeps
        // file1 is written
        // runar stops runartest
        // file2 is written
        // runar clears second filewrite from buffer
        // runar starts runartest
        // TODO broken because runar does not handle the interrupt properly
        assert.stdout("start foo\nstart foo\n").interrupted();
    }

    #[cfg(failing_tests)]
    #[test]
    fn file_watch_with_child_sleep() {
        let tmp_dir = TempDir::new().unwrap();
        let tmp_file = tmp_dir.child("file");
        tmp_file.touch().unwrap();
        let file = tmp_file.path().to_str().unwrap().to_owned();

        delayed_write_file(200, tmp_file);

        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-k", "10", &testprog("foo waitchild bar sleep"), &file])
            .timeout(Duration::from_millis(500))
            .assert();

        // runar starts runartest foo
        // foo starts child bar
        // foo waits on bar
        // bar sleeps
        // file is written
        // runar stops foo
        // runar cleans up bar
        // runar restarts foo
        // TODO broken because runar does not handle the interrupt properly
        assert.stdout("start foo\nstart bar\nstart foo\nstart bar\n").interrupted();
    }

    #[cfg(failing_tests)]
    #[test]
    fn file_watch_with_child_hang() {
        let tmp_dir = TempDir::new().unwrap();
        let tmp_file = tmp_dir.child("file");
        tmp_file.touch().unwrap();
        let file = tmp_file.path().to_str().unwrap().to_owned();

        delayed_write_file(200, tmp_file);

        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-k", "10", &testprog("foo waitchild sleep hang"), &file])
            .timeout(Duration::from_millis(500))
            .assert();

        // runar starts runartest foo
        // foo starts child bar
        // foo waits on bar
        // bar hangs
        // file is written
        // runar stops foo
        // runar cleans up bar
        // runar restarts foo
        // TODO broken because runar does not handle the interrupt properly
        assert.stdout("start foo\nstart bar\nstart foo\nstart bar\n").interrupted();
    }

    #[cfg(failing_tests)]
    #[test]
    fn file_watch_with_grandchild() {
        let tmp_dir = TempDir::new().unwrap();
        let tmp_file = tmp_dir.child("file");
        tmp_file.touch().unwrap();
        let file = tmp_file.path().to_str().unwrap().to_owned();

        delayed_write_file(200, tmp_file);

        let assert = Command::cargo_bin("runar")
            .unwrap()
            .args(["-k", "10", &testprog("foo child bar sleep"), &file])
            .timeout(Duration::from_millis(500))
            .assert();

        // runar starts runartest foo
        // foo starts child bar
        // foo exits
        // bar sleeps
        // file is written
        // runar attempts to stop foo
        // runar cleans up bar
        // runar restarts foo
        // TODO broken because runar does not handle the interrupt properly
        assert.stdout("start foo\nstart bar\nstart foo\nstart bar\n").interrupted();
    }
}
