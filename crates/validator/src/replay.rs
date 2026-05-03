use crate::runner::RunOutcome;
use crate::spec::Spec;
use anyhow::Result;
use std::path::Path;

/// Writes a replay.toml that reproduces the same step sequence. For now we
/// just clone the original spec — future versions can transcribe recorded
/// commands so non-deterministic specs (e.g. ones that branch on detected
/// windows) can still be replayed deterministically.
pub fn write(out_dir: &Path, spec: &Spec, outcome: &RunOutcome) -> Result<()> {
    let path = out_dir.join(format!("{}.replay.toml", outcome.spec_name));
    std::fs::write(path, toml::to_string_pretty(spec)?)?;
    Ok(())
}
