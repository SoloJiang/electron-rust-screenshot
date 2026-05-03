use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "validator",
    about = "Drives the screenshot engine via TOML specs"
)]
pub struct Cli {
    /// One or more TOML spec files to run sequentially.
    #[arg(required = true)]
    pub specs: Vec<PathBuf>,

    /// Path to the engine binary (override). Defaults to scripts/run-engine.js.
    #[arg(long)]
    pub engine: Option<PathBuf>,

    /// Directory for report.json + replay.toml + diagnostics dumps.
    #[arg(long, default_value = "validator-output")]
    pub out: PathBuf,

    /// Stop on first spec failure (default: continue and aggregate).
    #[arg(long)]
    pub fail_fast: bool,

    /// Tier override: scripted | real (default: respect each spec's [meta]).
    #[arg(long)]
    pub tier: Option<String>,
}
