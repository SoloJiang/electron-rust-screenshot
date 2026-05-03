use crate::asserts;
use crate::child::EngineChild;
use crate::client::Client;
use crate::matcher;
use crate::spec::{Assert, Spec, Step};
use crate::timeline::Timeline;
use anyhow::{anyhow, Result};
use harness_protocol::{Command, MouseButton, ServerMessage, Tier};
use interprocess::TryClone;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub struct RunOutcome {
    pub spec_name: String,
    pub passed: bool,
    pub failures: Vec<String>,
    pub timeline: Timeline,
    pub duration: Duration,
}

pub fn run(
    spec: &Spec,
    engine_path: Option<&str>,
    tier_override: Option<&str>,
) -> Result<RunOutcome> {
    let setup_json = setup_to_json(spec);
    let child = EngineChild::spawn(&setup_json, engine_path)?;
    let stream = child.stream.try_clone()?;
    let mut client = Client::new(stream)?;

    let timeline = Arc::new(Mutex::new(Timeline::default()));
    let timeline_for_thread = Arc::clone(&timeline);
    let stream_for_thread = child.stream.try_clone()?;
    let recv_thread = thread::spawn(move || -> Result<()> {
        let mut local_client = Client::new(stream_for_thread)?;
        loop {
            match local_client.try_recv() {
                Ok(Some(msg)) => timeline_for_thread.lock().unwrap().record(msg),
                Ok(None) => break,
                Err(e) => {
                    if e.downcast_ref::<std::io::Error>()
                        .is_some_and(|io| io.kind() == std::io::ErrorKind::WouldBlock)
                    {
                        thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    log::warn!("recv error: {e}");
                    break;
                }
            }
        }
        Ok(())
    });

    // Wait for hello
    let hello_deadline = Instant::now() + Duration::from_secs(2);
    let mut hello_seen = false;
    while Instant::now() < hello_deadline {
        let tl = timeline.lock().unwrap();
        if tl
            .entries
            .iter()
            .any(|e| matches!(e.message, ServerMessage::Hello { .. }))
        {
            hello_seen = true;
            break;
        }
        drop(tl);
        thread::sleep(Duration::from_millis(20));
    }
    if !hello_seen {
        return Err(anyhow!("engine did not send hello within 2s"));
    }

    // Drive steps
    let started = Instant::now();
    let spec_deadline = started + Duration::from_millis(spec.meta.timeout_ms);
    let mut seq = 1u64;
    let mut last_seq = 0u64;
    for step in &spec.steps {
        if Instant::now() > spec_deadline {
            return Err(anyhow!(
                "spec timeout exceeded ({} ms)",
                spec.meta.timeout_ms
            ));
        }
        let tier_default = tier_override.unwrap_or(&spec.meta.tier);
        match translate_step(step, &mut seq, tier_default) {
            Ok(Some(StepAction::Send(cmd))) => {
                let cmd_seq = cmd.seq();
                client.send(&cmd)?;
                let ack_deadline = Instant::now() + Duration::from_secs(2);
                let mut ack_ok = false;
                let mut ack_error = None;
                let mut ack_seq = last_seq;
                while Instant::now() < ack_deadline {
                    let tl = timeline.lock().unwrap();
                    if let Some(ack) = tl.entries.iter().rev().find_map(|e| match &e.message {
                        ServerMessage::CommandAck(ack) if ack.ref_seq == cmd_seq => Some(ack),
                        _ => None,
                    }) {
                        ack_ok = ack.ok;
                        ack_error = ack.error.clone();
                        ack_seq = ack.seq;
                        break;
                    }
                    drop(tl);
                    thread::sleep(Duration::from_millis(10));
                }
                if !ack_ok {
                    return Err(anyhow!(
                        "command seq={cmd_seq} failed: {}",
                        ack_error.unwrap_or_else(|| "no ack received".into())
                    ));
                }
                last_seq = ack_seq;
            }
            Ok(Some(StepAction::WaitFor { event, timeout_ms })) => {
                let deadline = Instant::now() + Duration::from_millis(timeout_ms);
                let mut found = false;
                while Instant::now() < deadline {
                    let tl = timeline.lock().unwrap();
                    if tl
                        .entries
                        .iter()
                        .filter(|e| e.message.seq() > last_seq)
                        .filter_map(|e| match &e.message {
                            ServerMessage::EngineEvent { payload, .. } => Some(payload),
                            _ => None,
                        })
                        .any(|ev| event_matches_name(ev, &event))
                    {
                        found = true;
                        break;
                    }
                    drop(tl);
                    thread::sleep(Duration::from_millis(10));
                }
                if !found {
                    return Err(anyhow!(
                        "wait_for '{}' timed out after {} ms",
                        event,
                        timeout_ms
                    ));
                }
                {
                    let tl = timeline.lock().unwrap();
                    last_seq = tl
                        .entries
                        .iter()
                        .map(|e| e.message.seq())
                        .max()
                        .unwrap_or(last_seq);
                }
            }
            Ok(Some(StepAction::Sleep(ms))) => {
                thread::sleep(Duration::from_millis(ms));
                {
                    let tl = timeline.lock().unwrap();
                    last_seq = tl
                        .entries
                        .iter()
                        .map(|e| e.message.seq())
                        .max()
                        .unwrap_or(last_seq);
                }
            }
            Ok(None) => {}
            Err(e) => return Err(anyhow!(e)),
        }
    }

    // Allow some final events (regionSelected, saved) to drain
    thread::sleep(Duration::from_millis(150));

    // Tear down
    drop(client);
    child.shutdown();
    let _ = recv_thread.join();

    // Evaluate asserts
    let timeline_final = Arc::try_unwrap(timeline)
        .map_err(|_| anyhow!("timeline still shared"))?
        .into_inner()
        .unwrap();
    let mut failures = Vec::new();
    for a in &spec.asserts {
        if let Err(reason) = evaluate_assert(a, &timeline_final, started) {
            failures.push(reason);
        }
    }
    Ok(RunOutcome {
        spec_name: spec.meta.name.clone(),
        passed: failures.is_empty(),
        failures,
        timeline: timeline_final,
        duration: started.elapsed(),
    })
}

fn setup_to_json(spec: &Spec) -> String {
    let mut obj = serde_json::Map::new();
    if let Some(p) = &spec.setup.save_path {
        obj.insert("savePath".into(), serde_json::Value::String(p.clone()));
    }
    if let Some(f) = &spec.setup.format {
        obj.insert("format".into(), serde_json::Value::String(f.clone()));
    }
    if let Some(q) = spec.setup.quality {
        obj.insert("quality".into(), serde_json::Value::Number(q.into()));
    }
    if let Some(c) = &spec.setup.default_color {
        obj.insert("defaultColor".into(), serde_json::Value::String(c.clone()));
    }
    if let Some(s) = spec.setup.default_size {
        obj.insert("defaultSize".into(), serde_json::Value::Number(s.into()));
    }
    serde_json::Value::Object(obj).to_string()
}

enum StepAction {
    Send(Command),
    WaitFor { event: String, timeout_ms: u64 },
    Sleep(u64),
}

fn translate_step(
    step: &Step,
    seq: &mut u64,
    tier_default: &str,
) -> Result<Option<StepAction>, String> {
    let s = *seq;
    *seq += 1;
    let mode = |m: &str| -> Result<Tier, String> {
        let effective = if m.is_empty() { tier_default } else { m };
        match effective {
            "real" => Ok(Tier::Real),
            "scripted" => Ok(Tier::Scripted),
            _ => Err(format!("invalid mode: '{effective}'")),
        }
    };
    match step {
        Step::MouseDown {
            x,
            y,
            button,
            modifiers,
            mode: m,
        } => Ok(Some(StepAction::Send(Command::MouseDown {
            seq: s,
            x: *x,
            y: *y,
            button: parse_button(button)?,
            modifiers: modifiers.clone(),
            mode: mode(m)?,
        }))),
        Step::MouseMove { x, y, mode: m } => Ok(Some(StepAction::Send(Command::MouseMove {
            seq: s,
            x: *x,
            y: *y,
            mode: mode(m)?,
        }))),
        Step::MouseUp {
            x,
            y,
            button,
            modifiers,
            mode: m,
        } => Ok(Some(StepAction::Send(Command::MouseUp {
            seq: s,
            x: *x,
            y: *y,
            button: parse_button(button)?,
            modifiers: modifiers.clone(),
            mode: mode(m)?,
        }))),
        Step::Drag {
            from,
            to,
            button,
            modifiers,
            mode: m,
        } => Ok(Some(StepAction::Send(Command::Drag {
            seq: s,
            from: harness_protocol::DragPoint {
                x: from[0],
                y: from[1],
            },
            to: harness_protocol::DragPoint { x: to[0], y: to[1] },
            button: parse_button(button)?,
            modifiers: modifiers.clone(),
            mode: mode(m)?,
        }))),
        Step::KeyPress {
            key,
            modifiers,
            mode: m,
        } => Ok(Some(StepAction::Send(Command::KeyPress {
            seq: s,
            key: key.clone(),
            modifiers: modifiers.clone(),
            mode: mode(m)?,
        }))),
        Step::TextInput { text, mode: m } => Ok(Some(StepAction::Send(Command::TextInput {
            seq: s,
            text: text.clone(),
            mode: mode(m)?,
        }))),
        Step::ToolSet { tool, mode: m } => Ok(Some(StepAction::Send(Command::ToolSet {
            seq: s,
            tool: tool.clone(),
            mode: mode(m)?,
        }))),
        Step::Save { mode: m } => Ok(Some(StepAction::Send(Command::Save {
            seq: s,
            mode: mode(m)?,
        }))),
        Step::Cancel { mode: m } => Ok(Some(StepAction::Send(Command::Cancel {
            seq: s,
            mode: mode(m)?,
        }))),
        Step::SnapshotRequest => Ok(Some(StepAction::Send(Command::SnapshotRequest { seq: s }))),
        Step::Composite { save_path, format } => {
            Ok(Some(StepAction::Send(Command::CompositeRequest {
                seq: s,
                save_path: save_path.clone(),
                format: format.clone(),
            })))
        }
        Step::Sleep { ms } => Ok(Some(StepAction::Sleep(*ms))),
        Step::WaitFor { event, timeout_ms } => Ok(Some(StepAction::WaitFor {
            event: event.clone(),
            timeout_ms: *timeout_ms,
        })),
    }
}

fn parse_button(s: &str) -> Result<MouseButton, String> {
    match s {
        "right" => Ok(MouseButton::Right),
        "middle" => Ok(MouseButton::Middle),
        "left" => Ok(MouseButton::Left),
        _ => Err(format!("invalid button: '{s}'")),
    }
}

fn event_matches_name(ev: &serde_json::Value, name: &str) -> bool {
    ev.get("type")
        .and_then(|v| v.as_str())
        .is_some_and(|t| t.eq_ignore_ascii_case(name))
}

fn evaluate_assert(a: &Assert, tl: &Timeline, started: Instant) -> Result<(), String> {
    match a {
        Assert::EventEmitted { event, matches } => {
            let actual_json: Vec<&serde_json::Value> = tl
                .engine_events()
                .filter(|ev| event_matches_name(ev, event))
                .collect();
            if actual_json.is_empty() {
                return Err(format!("event_emitted: no '{event}' event observed"));
            }
            if let Some(expected) = matches {
                let any_matched = actual_json.iter().any(|j| matcher::partial_eq(expected, j));
                if !any_matched {
                    return Err(format!(
                        "event_emitted: '{event}' observed but no payload matched the expectation"
                    ));
                }
            }
            Ok(())
        }
        Assert::NoEvent { event, within_ms } => {
            let found = if let Some(ms) = within_ms {
                let cutoff = started + Duration::from_millis(*ms);
                tl.entries.iter().any(|e| {
                    if e.at > cutoff {
                        return false;
                    }
                    match &e.message {
                        ServerMessage::EngineEvent { payload, .. } => {
                            event_matches_name(payload, event)
                        }
                        _ => false,
                    }
                })
            } else {
                tl.engine_events().any(|ev| event_matches_name(ev, event))
            };
            if found {
                Err(format!("no_event: forbidden event '{event}' was emitted"))
            } else {
                Ok(())
            }
        }
        Assert::ArtifactExists { path } => {
            if std::path::Path::new(path).exists() {
                Ok(())
            } else {
                Err(format!("artifact_exists: '{path}' missing"))
            }
        }
        Assert::ArtifactDimensions {
            path,
            width,
            height,
        } => asserts::dimensions::check(path, *width, *height),
        Assert::ArtifactHash { path, sha256 } => asserts::hash::check(path, sha256),
        Assert::ArtifactPixelDiff {
            path,
            against,
            max_diff_ratio,
        } => asserts::pixel_diff::check(path, against, *max_diff_ratio),
        Assert::Performance { metric, max_ms } => asserts::performance::check(tl, metric, *max_ms),
        Assert::StateAt { step, .. } => Err(format!(
            "state_at assertion on step '{step}' not yet implemented"
        )),
    }
}
