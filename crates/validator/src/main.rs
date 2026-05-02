mod cli;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    env_logger::init();
    let args = cli::Cli::parse();
    log::info!("validator: {} spec(s)", args.specs.len());
    for spec_path in &args.specs {
        log::info!("would run spec: {}", spec_path.display());
    }
    Ok(())
}
