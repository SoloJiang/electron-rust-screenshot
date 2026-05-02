use crate::snapshot::StateSnapshot;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandAck {
    pub ts: u64,
    pub seq: u64,
    #[serde(rename = "refSeq")]
    pub ref_seq: u64,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Hello {
        ts: u64,
        seq: u64,
        version: u32,
        platform: String,
    },
    EngineEvent {
        ts: u64,
        seq: u64,
        payload: Value,
    },
    StateSnapshot {
        ts: u64,
        seq: u64,
        payload: StateSnapshot,
    },
    CommandAck(CommandAck),
}
