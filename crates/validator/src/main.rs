#![allow(dead_code)]

mod asserts;
mod child;
mod cli;
mod client;
mod matcher;
mod replay;
mod report;
mod runner;
pub mod spec;
mod timeline;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    env_logger::init();
    let args = cli::Cli::parse();
    std::fs::create_dir_all(&args.out)?;
    let mut all_passed = true;
    for spec_path in &args.specs {
        let spec = spec::load(spec_path)?;
        log::info!("running spec: {} ({})", spec.meta.name, spec_path.display());
        match runner::run(&spec) {
            Ok(outcome) => {
                report::write(&args.out, &outcome)?;
                replay::write(&args.out, &spec, &outcome)?;
                if !outcome.passed {
                    all_passed = false;
                    log::error!(
                        "FAIL {} ({} failures)",
                        outcome.spec_name,
                        outcome.failures.len()
                    );
                    for f in &outcome.failures {
                        log::error!("  - {f}");
                    }
                    if args.fail_fast {
                        break;
                    }
                } else {
                    log::info!("PASS {}", outcome.spec_name);
                }
            }
            Err(e) => {
                log::error!("spec {} crashed: {e}", spec.meta.name);
                all_passed = false;
                if args.fail_fast {
                    break;
                }
            }
        }
    }
    std::process::exit(if all_passed { 0 } else { 1 });
}
