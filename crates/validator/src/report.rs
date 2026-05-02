use crate::runner::RunOutcome;
use anyhow::Result;
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
struct Report<'a> {
    spec: &'a str,
    passed: bool,
    duration_ms: u128,
    failures: &'a [String],
    timeline: Vec<TimelineRow>,
}

#[derive(Serialize)]
struct TimelineRow {
    at_ms: u128,
    message: serde_json::Value,
}

pub fn write(out_dir: &Path, outcome: &RunOutcome) -> Result<()> {
    let zero = outcome
        .timeline
        .entries
        .first()
        .map(|e| e.at)
        .unwrap_or_else(std::time::Instant::now);
    let timeline: Vec<TimelineRow> = outcome
        .timeline
        .entries
        .iter()
        .map(|e| TimelineRow {
            at_ms: e.at.duration_since(zero).as_millis(),
            message: serde_json::to_value(&e.message).unwrap(),
        })
        .collect();
    let report = Report {
        spec: &outcome.spec_name,
        passed: outcome.passed,
        duration_ms: outcome.duration.as_millis(),
        failures: &outcome.failures,
        timeline,
    };
    let path = out_dir.join(format!("{}.report.json", outcome.spec_name));
    std::fs::write(path, serde_json::to_string_pretty(&report)?)?;
    Ok(())
}
