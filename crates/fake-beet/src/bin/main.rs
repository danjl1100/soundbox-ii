//! Binary for faking `beet` in end-to-end integration tests

use std::process::ExitCode;

use eyre::Context as _;
use fake_beet::ConfigAll;

fn main() -> eyre::Result<ExitCode> {
    let config_file = get_env("FAKE_BEET_CONFIG_FILE")?;
    let config_str = std::fs::read_to_string(&config_file)
        .with_context(|| format!("invalid fake-beet config file path: {config_file}"))?;

    let config_all: ConfigAll = serde_json::from_str(&config_str)
        .with_context(|| format!("invalid fake-beet config: {config_str:?}"))?;

    let args: Vec<_> = std::env::args().skip(1).collect();
    let Some(config) = config_all.into_configs_map().remove(&args) else {
        eyre::bail!("unknown fake-beet args: {args:?}")
    };

    let exit_code = config.execute();
    Ok(exit_code)
}

fn get_env(var: &str) -> eyre::Result<String> {
    std::env::var(var).with_context(|| format!("missing required fake-beet env var: {var}"))
}
