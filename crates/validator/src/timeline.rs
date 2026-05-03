use harness_protocol::{ServerMessage, StateSnapshot};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct TimelineEntry {
    pub at: Instant,
    pub message: ServerMessage,
}

#[derive(Default)]
pub struct Timeline {
    pub entries: Vec<TimelineEntry>,
}

impl Timeline {
    pub fn record(&mut self, msg: ServerMessage) {
        self.entries.push(TimelineEntry {
            at: Instant::now(),
            message: msg,
        });
    }

    /// Iterator over engine-event payloads (serde_json::Value).
    pub fn engine_events(&self) -> impl Iterator<Item = &serde_json::Value> {
        self.entries.iter().filter_map(|e| match &e.message {
            ServerMessage::EngineEvent { payload, .. } => Some(payload),
            _ => None,
        })
    }

    pub fn last_state_snapshot(&self) -> Option<&StateSnapshot> {
        self.entries.iter().rev().find_map(|e| match &e.message {
            ServerMessage::StateSnapshot { payload, .. } => Some(payload),
            _ => None,
        })
    }
}
