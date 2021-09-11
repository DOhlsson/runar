extern crate assert_cli;

mod integration {
    use assert_cli::Assert;

    #[test]
    fn without_args_fails() {
        Assert::main_binary()
            .fails()
            .unwrap();
    }

    #[test]
    fn with_only_cmd_arg_fails() {
        Assert::main_binary()
            .with_args(&["sleep 2s"])
            .fails()
            .unwrap();
    }

    #[test]
    fn with_exit_flag() {
        Assert::main_binary()
            .with_args(&["-e", "sleep 1s", "."])
            .unwrap();
    }
}
