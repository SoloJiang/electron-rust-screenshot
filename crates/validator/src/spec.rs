use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Spec {
    pub meta: Meta,
    #[serde(default)]
    pub setup: Setup,
    #[serde(default)]
    pub steps: Vec<Step>,
    #[serde(default)]
    pub asserts: Vec<Assert>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Meta {
    pub name: String,
    #[serde(default = "default_tier")]
    pub tier: String, // "scripted" | "real" | "hybrid"
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub description: Option<String>,
}
fn default_tier() -> String {
    "scripted".to_string()
}
fn default_timeout() -> u64 {
    10_000
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct Setup {
    #[serde(default)]
    pub save_path: Option<String>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub quality: Option<u8>,
    #[serde(default)]
    pub default_color: Option<String>,
    #[serde(default)]
    pub default_size: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Step {
    MouseDown {
        x: f64,
        y: f64,
        #[serde(default = "default_button")]
        button: String,
        #[serde(default)]
        modifiers: Vec<String>,
        #[serde(default = "default_mode")]
        mode: String,
    },
    MouseMove {
        x: f64,
        y: f64,
        #[serde(default = "default_mode")]
        mode: String,
    },
    MouseUp {
        x: f64,
        y: f64,
        #[serde(default = "default_button")]
        button: String,
        #[serde(default)]
        modifiers: Vec<String>,
        #[serde(default = "default_mode")]
        mode: String,
    },
    Drag {
        from: [f64; 2],
        to: [f64; 2],
        #[serde(default = "default_button")]
        button: String,
        #[serde(default)]
        modifiers: Vec<String>,
        #[serde(default = "default_mode")]
        mode: String,
    },
    KeyPress {
        key: String,
        #[serde(default)]
        modifiers: Vec<String>,
        #[serde(default = "default_mode")]
        mode: String,
    },
    TextInput {
        text: String,
        #[serde(default = "default_mode")]
        mode: String,
    },
    ToolSet {
        tool: String,
        #[serde(default = "default_mode")]
        mode: String,
    },
    Save {
        #[serde(default = "default_mode")]
        mode: String,
    },
    Cancel {
        #[serde(default = "default_mode")]
        mode: String,
    },
    SnapshotRequest,
    Composite {
        save_path: String,
        #[serde(default = "default_format")]
        format: String,
    },
    Sleep {
        ms: u64,
    },
    WaitFor {
        event: String,
        #[serde(default = "default_wait_timeout")]
        timeout_ms: u64,
    },
}
fn default_button() -> String {
    "left".to_string()
}
fn default_mode() -> String {
    "scripted".to_string()
}
fn default_format() -> String {
    "png".to_string()
}
fn default_wait_timeout() -> u64 {
    2_000
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Assert {
    EventEmitted {
        event: String,
        #[serde(default)]
        matches: Option<toml::Value>,
    },
    StateAt {
        step: String,
        expect: toml::Value,
    },
    ArtifactExists {
        path: String,
    },
    ArtifactDimensions {
        path: String,
        width: u32,
        height: u32,
    },
    ArtifactHash {
        path: String,
        sha256: String,
    },
    ArtifactPixelDiff {
        path: String,
        against: String,
        max_diff_ratio: f64,
    },
    Performance {
        metric: String,
        max_ms: f64,
    },
    NoEvent {
        event: String,
        #[serde(default)]
        within_ms: Option<u64>,
    },
}

pub fn load(path: &Path) -> anyhow::Result<Spec> {
    let raw = std::fs::read_to_string(path)?;
    let spec: Spec = toml::from_str(&raw)?;
    Ok(spec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_spec() {
        let raw = r#"
[meta]
name = "smoke_cancel"

[[steps]]
kind = "key_press"
key = "Escape"

[[asserts]]
kind = "event_emitted"
event = "cancelled"
"#;
        let spec: Spec = toml::from_str(raw).unwrap();
        assert_eq!(spec.meta.name, "smoke_cancel");
        assert_eq!(spec.meta.tier, "scripted");
        assert_eq!(spec.steps.len(), 1);
        assert!(matches!(&spec.steps[0], Step::KeyPress { key, .. } if key == "Escape"));
        assert!(
            matches!(&spec.asserts[0], Assert::EventEmitted { event, .. } if event == "cancelled")
        );
    }

    #[test]
    fn parses_drag_with_modifiers() {
        let raw = r#"
[meta]
name = "drag_with_shift"

[[steps]]
kind = "drag"
from = [50, 60]
to = [150, 160]
modifiers = ["Shift"]
"#;
        let spec: Spec = toml::from_str(raw).unwrap();
        match &spec.steps[0] {
            Step::Drag {
                from,
                to,
                modifiers,
                ..
            } => {
                assert_eq!(from, &[50.0, 60.0]);
                assert_eq!(to, &[150.0, 160.0]);
                assert_eq!(modifiers, &vec!["Shift".to_string()]);
            }
            _ => panic!("expected Drag step"),
        }
    }

    #[test]
    fn parses_assertions_for_artifacts_and_perf() {
        let raw = r#"
[meta]
name = "artifact_check"

[[asserts]]
kind = "artifact_exists"
path = "/tmp/x.png"

[[asserts]]
kind = "artifact_dimensions"
path = "/tmp/x.png"
width = 100
height = 100

[[asserts]]
kind = "performance"
metric = "hit_test_ms_p99"
max_ms = 5.0
"#;
        let spec: Spec = toml::from_str(raw).unwrap();
        assert_eq!(spec.asserts.len(), 3);
    }
}
