//! Binary for faking `beet` in end-to-end integration tests

fn main() -> eyre::Result<std::process::ExitCode> {
    fake_beet::fake_beet_main()
}
