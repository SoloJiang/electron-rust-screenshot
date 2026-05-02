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

pub fn run(spec: &Spec) -> Result<RunOutcome> {
    let setup_json = setup_to_json(spec);
    let child = EngineChild::spawn(&setup_json)?;
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
                    log::warn!("recv error: {e}");
                    break;
                }
            }
        }
        Ok(())
    });

    // Wait for hello
    let hello_deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < hello_deadline {
        let tl = timeline.lock().unwrap();
        if tl
            .entries
            .iter()
            .any(|e| matches!(e.message, ServerMessage::Hello { .. }))
        {
            break;
        }
        drop(tl);
        thread::sleep(Duration::from_millis(20));
    }

    // Drive steps
    let started = Instant::now();
    let mut seq = 1u64;
    for step in &spec.steps {
        match translate_step(step, &mut seq) {
            Some(StepAction::Send(cmd)) => client.send(&cmd)?,
            Some(StepAction::WaitFor { event, timeout_ms }) => {
                let deadline = Instant::now() + Duration::from_millis(timeout_ms);
                while Instant::now() < deadline {
                    let tl = timeline.lock().unwrap();
                    if tl.engine_events().any(|ev| event_matches_name(ev, &event)) {
                        break;
                    }
                    drop(tl);
                    thread::sleep(Duration::from_millis(10));
                }
            }
            Some(StepAction::Sleep(ms)) => thread::sleep(Duration::from_millis(ms)),
            None => {}
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
        if let Err(reason) = evaluate_assert(a, &timeline_final) {
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
        obj.insert(
            "quality".into(),
            serde_json::Value::Number(q.into()),
        );
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

fn translate_step(step: &Step, seq: &mut u64) -> Option<StepAction> {
    let s = *seq;
    *seq += 1;
    let mode = |m: &str| if m == "real" { Tier::Real } else { Tier::Scripted };
    match step {
        Step::MouseDown {
            x,
            y,
            button,
            modifiers,
            mode: m,
        } => Some(StepAction::Send(Command::MouseDown {
            seq: s,
            x: *x,
            y: *y,
            button: parse_button(button),
            modifiers: modifiers.clone(),
            mode: mode(m),
        })),
        Step::MouseMove { x, y, mode: m } => Some(StepAction::Send(Command::MouseMove {
            seq: s,
            x: *x,
            y: *y,
            mode: mode(m),
        })),
        Step::MouseUp {
            x,
            y,
            button,
            modifiers,
            mode: m,
        } => Some(StepAction::Send(Command::MouseUp {
            seq: s,
            x: *x,
            y: *y,
            button: parse_button(button),
            modifiers: modifiers.clone(),
            mode: mode(m),
        })),
        Step::Drag {
            from,
            to,
            button,
            modifiers,
            mode: m,
        } => Some(StepAction::Send(Command::Drag {
            seq: s,
            from: harness_protocol::DragPoint {
                x: from[0],
                y: from[1],
            },
            to: harness_protocol::DragPoint {
                x: to[0],
                y: to[1],
            },
            button: parse_button(button),
            modifiers: modifiers.clone(),
            mode: mode(m),
        })),
        Step::KeyPress {
            key,
            modifiers,
            mode: m,
        } => Some(StepAction::Send(Command::KeyPress {
            seq: s,
            key: key.clone(),
            modifiers: modifiers.clone(),
            mode: mode(m),
        })),
        Step::TextInput { text, mode: m } => Some(StepAction::Send(Command::TextInput {
            seq: s,
            text: text.clone(),
            mode: mode(m),
        })),
        Step::ToolSet { tool, mode: m } => Some(StepAction::Send(Command::ToolSet {
            seq: s,
            tool: tool.clone(),
            mode: mode(m),
        })),
        Step::Save { mode: m } => Some(StepAction::Send(Command::Save {
            seq: s,
            mode: mode(m),
        })),
        Step::Cancel { mode: m } => Some(StepAction::Send(Command::Cancel {
            seq: s,
            mode: mode(m),
        })),
        Step::SnapshotRequest => Some(StepAction::Send(Command::SnapshotRequest { seq: s })),
        Step::Composite { save_path, format } => Some(StepAction::Send(Command::CompositeRequest {
            seq: s,
            save_path: save_path.clone(),
            format: format.clone(),
        })),
        Step::Sleep { ms } => Some(StepAction::Sleep(*ms)),
        Step::WaitFor { event, timeout_ms } => Some(StepAction::WaitFor {
            event: event.clone(),
            timeout_ms: *timeout_ms,
        }),
    }
}

fn parse_button(s: &str) -> MouseButton {
    match s {
        "right" => MouseButton::Right,
        "middle" => MouseButton::Middle,
        _ => MouseButton::Left,
    }
}

fn event_matches_name(ev: &serde_json::Value, name: &str) -> bool {
    ev.get("type")
        .and_then(|v| v.as_str())
        .is_some_and(|t| t.eq_ignore_ascii_case(name))
}

fn evaluate_assert(a: &Assert, tl: &Timeline) -> Result<(), String> {
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
                let any_matched = actual_json
                    .iter()
                    .any(|j| matcher::partial_eq(expected, j));
                if !any_matched {
                    return Err(format!(
                        "event_emitted: '{event}' observed but no payload matched the expectation"
                    ));
                }
            }
            Ok(())
        }
        Assert::NoEvent { event, .. } => {
            if tl.engine_events().any(|ev| event_matches_name(ev, event)) {
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
        // Dimensions, Hash, PixelDiff, Performance — implemented in M4.
        _ => Ok(()),
    }
}
