extern crate assert_cli;

mod integration {
    use assert_cli::Assert;

    #[test]
    fn calling_without_args() {
        Assert::main_binary()
            .fails()
            .unwrap();
    }
}
