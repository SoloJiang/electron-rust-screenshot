use crate::timeline::Timeline;

/// Maps user-friendly metric names to the camelCase JSON keys emitted by the engine.
fn metric_key(name: &str) -> String {
    match name {
        "capture_ms" | "capture" => "captureMs".into(),
        "window_enum_ms" | "window_enum" => "windowEnumMs".into(),
        "hit_test_p99_ms" | "hit_test_ms_p99" => "hitTestP99Ms".into(),
        "frame_time_ms" | "frame_time" => "frameTimeMs".into(),
        "egui_paint_ms" | "egui_paint" => "eguiPaintMs".into(),
        "memory_mb" | "memory" => "memoryMb".into(),
        other => other.into(),
    }
}

pub fn check(tl: &Timeline, metric: &str, max_ms: f64) -> Result<(), String> {
    let key = metric_key(metric);
    let values: Vec<f64> = tl
        .engine_events()
        .filter(|ev| {
            ev.get("type")
                .and_then(|v| v.as_str())
                .is_some_and(|t| t.eq_ignore_ascii_case("metrics"))
        })
        .filter_map(|ev| ev.get(&key).and_then(|v| v.as_f64()))
        .collect();

    if values.is_empty() {
        return Err(format!(
            "performance: no metrics event with key '{metric}' (json key: {key}) found"
        ));
    }

    let max_val = values.iter().fold(f64::NEG_INFINITY, |a, b| a.max(*b));
    if max_val > max_ms {
        Err(format!(
            "performance: metric '{metric}' max value {max_val:.2} ms exceeds threshold {max_ms} ms"
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::Timeline;
    use harness_protocol::ServerMessage;

    fn make_timeline_with_metrics(capture_ms: f64) -> Timeline {
        let mut tl = Timeline::default();
        tl.record(ServerMessage::EngineEvent {
            ts: 0,
            seq: 0,
            payload: serde_json::json!({
                "type": "metrics",
                "captureMs": capture_ms,
                "windowEnumMs": 5.0,
                "hitTestP99Ms": 1.0,
                "frameTimeMs": 16.0,
                "eguiPaintMs": 2.0,
                "memoryMb": 50.0,
            }),
        });
        tl
    }

    #[test]
    fn metric_within_threshold_passes() {
        let tl = make_timeline_with_metrics(10.0);
        assert!(check(&tl, "capture_ms", 20.0).is_ok());
    }

    #[test]
    fn metric_exceeds_threshold_fails() {
        let tl = make_timeline_with_metrics(30.0);
        assert!(check(&tl, "capture_ms", 20.0).is_err());
    }

    #[test]
    fn unknown_metric_fails() {
        let tl = make_timeline_with_metrics(10.0);
        assert!(check(&tl, "nonexistent", 100.0).is_err());
    }
}
