use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub state: String, // "Idle" | "Capturing" | "OverlayRunning" | "FreeSelecting" | "Editing" | "Saving"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<Value>, // Rect or null
    pub layers: usize,
    #[serde(rename = "activeTool")]
    pub active_tool: String,
    #[serde(
        default,
        rename = "hoveredWindow",
        skip_serializing_if = "Option::is_none"
    )]
    pub hovered_window: Option<Value>,
    #[serde(
        default,
        rename = "freeSelecting",
        skip_serializing_if = "Option::is_none"
    )]
    pub free_selecting: Option<Value>, // {"start":{},"current":{}}
}
