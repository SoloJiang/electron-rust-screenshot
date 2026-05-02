use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    #[default]
    Scripted,
    Real,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseButton {
    #[default]
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DragPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    Hello {
        seq: u64,
        version: u32,
        platform: String,
    },
    MouseDown {
        seq: u64,
        x: f64,
        y: f64,
        #[serde(default)]
        button: MouseButton,
        #[serde(default)]
        modifiers: Vec<String>,
        #[serde(default)]
        mode: Tier,
    },
    MouseMove {
        seq: u64,
        x: f64,
        y: f64,
        #[serde(default)]
        mode: Tier,
    },
    MouseUp {
        seq: u64,
        x: f64,
        y: f64,
        #[serde(default)]
        button: MouseButton,
        #[serde(default)]
        modifiers: Vec<String>,
        #[serde(default)]
        mode: Tier,
    },
    Drag {
        seq: u64,
        from: DragPoint,
        to: DragPoint,
        #[serde(default)]
        button: MouseButton,
        #[serde(default)]
        modifiers: Vec<String>,
        #[serde(default)]
        mode: Tier,
    },
    KeyPress {
        seq: u64,
        key: String,
        #[serde(default)]
        modifiers: Vec<String>,
        #[serde(default)]
        mode: Tier,
    },
    TextInput {
        seq: u64,
        text: String,
        #[serde(default)]
        mode: Tier,
    },
    ToolSet {
        seq: u64,
        tool: String,
        #[serde(default)]
        mode: Tier,
    },
    Save {
        seq: u64,
        #[serde(default)]
        mode: Tier,
    },
    Cancel {
        seq: u64,
        #[serde(default)]
        mode: Tier,
    },
    SnapshotRequest {
        seq: u64,
    },
    CompositeRequest {
        seq: u64,
        save_path: String,
        #[serde(default = "default_format")]
        format: String,
    },
    WindowEnumRefresh {
        seq: u64,
    },
    Shutdown {
        seq: u64,
        #[serde(default)]
        reason: Option<String>,
    },
}

fn default_format() -> String {
    "png".to_string()
}

impl Command {
    pub fn seq(&self) -> u64 {
        match self {
            Command::Hello { seq, .. }
            | Command::MouseDown { seq, .. }
            | Command::MouseMove { seq, .. }
            | Command::MouseUp { seq, .. }
            | Command::Drag { seq, .. }
            | Command::KeyPress { seq, .. }
            | Command::TextInput { seq, .. }
            | Command::ToolSet { seq, .. }
            | Command::Save { seq, .. }
            | Command::Cancel { seq, .. }
            | Command::SnapshotRequest { seq }
            | Command::CompositeRequest { seq, .. }
            | Command::WindowEnumRefresh { seq }
            | Command::Shutdown { seq, .. } => *seq,
        }
    }

    /// Tier for this command. Pure-protocol commands (Hello, Shutdown,
    /// SnapshotRequest, CompositeRequest, WindowEnumRefresh) are always
    /// scripted — they don't dispatch through the OS event queue.
    pub fn mode(&self) -> Tier {
        match self {
            Command::MouseDown { mode, .. }
            | Command::MouseMove { mode, .. }
            | Command::MouseUp { mode, .. }
            | Command::Drag { mode, .. }
            | Command::KeyPress { mode, .. }
            | Command::TextInput { mode, .. }
            | Command::ToolSet { mode, .. }
            | Command::Save { mode, .. }
            | Command::Cancel { mode, .. } => *mode,
            Command::Hello { .. }
            | Command::SnapshotRequest { .. }
            | Command::CompositeRequest { .. }
            | Command::WindowEnumRefresh { .. }
            | Command::Shutdown { .. } => Tier::Scripted,
        }
    }
}
