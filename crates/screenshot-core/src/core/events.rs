use super::types::{DetectedWindow, Rect, ScreenInfo};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum EngineEvent {
    Started {
        screens: Vec<ScreenInfo>,
    },
    WindowHovered {
        window: DetectedWindow,
    },
    RegionSelected {
        #[serde(rename = "screenId")]
        screen_id: String,
        rect: Rect,
    },
    Saved {
        path: String,
        copied: bool,
    },
    Cancelled,
    Error {
        code: ErrorCode,
        message: String,
    },
    Metrics(MetricsPayload),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    CaptureFailed,
    NoDisplay,
    PermissionDenied,
    SaveFailed,
    Unknown,
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorCode::CaptureFailed => write!(f, "CAPTURE_FAILED"),
            ErrorCode::NoDisplay => write!(f, "NO_DISPLAY"),
            ErrorCode::PermissionDenied => write!(f, "PERMISSION_DENIED"),
            ErrorCode::SaveFailed => write!(f, "SAVE_FAILED"),
            ErrorCode::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsPayload {
    pub capture_ms: f64,
    pub window_enum_ms: f64,
    pub hit_test_p99_ms: f64,
    pub frame_time_ms: f64,
    pub egui_paint_ms: f64,
    pub memory_mb: f64,
}

pub struct EventBus {
    sender: crossbeam_channel::Sender<EngineEvent>,
    receiver: crossbeam_channel::Receiver<EngineEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        Self { sender, receiver }
    }

    pub fn emit(&self, event: EngineEvent) {
        let _ = self.sender.send(event);
    }

    pub fn try_recv(&self) -> Option<EngineEvent> {
        self.receiver.try_recv().ok()
    }

    pub fn clone_sender(&self) -> crossbeam_channel::Sender<EngineEvent> {
        self.sender.clone()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_bus_roundtrip() {
        let bus = EventBus::new();
        bus.emit(EngineEvent::Cancelled);
        assert_eq!(bus.try_recv(), Some(EngineEvent::Cancelled));
        assert_eq!(bus.try_recv(), None);
    }
}
