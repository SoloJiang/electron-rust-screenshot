//! Real-tier injector for macOS — drives the system event queue via cliclick.

use crate::core::engine::Engine;
use crate::core::types::LogicalPoint;
use crate::harness::coords::logical_to_physical;
use crate::harness::injector_scripted::InjectorResult;
use crate::harness::modifiers::{cliclick_name, parse_all};
use std::path::PathBuf;
use std::process::Command as StdCommand;

pub trait Cliclick {
    fn run(&self, args: &[&str]) -> std::io::Result<()>;
}

pub struct CliclickBin {
    pub path: PathBuf,
}

impl CliclickBin {
    pub fn discover() -> Option<Self> {
        for candidate in ["/opt/homebrew/bin/cliclick", "/usr/local/bin/cliclick"] {
            let p = PathBuf::from(candidate);
            if p.exists() {
                return Some(Self { path: p });
            }
        }
        which::which("cliclick").ok().map(|p| Self { path: p })
    }
}

impl Cliclick for CliclickBin {
    fn run(&self, args: &[&str]) -> std::io::Result<()> {
        let status = StdCommand::new(&self.path).args(args).status()?;
        if !status.success() {
            return Err(std::io::Error::other(format!(
                "cliclick exited with status {}",
                status
            )));
        }
        Ok(())
    }
}

/// Constructs a cliclick command argument for a "click and drag" sequence.
pub fn drag_args(from: (i32, i32), to: (i32, i32)) -> Vec<String> {
    vec![
        format!("dd:{},{}", from.0, from.1),
        format!("du:{},{}", to.0, to.1),
    ]
}

/// Constructs cliclick args for a single key press.
pub fn key_press_args(key: &str) -> Vec<String> {
    vec![format!("kp:{}", cliclick_key_name(key))]
}

fn cliclick_key_name(key: &str) -> &str {
    match key {
        "Escape" | "Esc" => "esc",
        "Enter" | "Return" => "return",
        "Space" => "space",
        "Tab" => "tab",
        "Backspace" => "delete",
        "ArrowUp" => "arrow-up",
        "ArrowDown" => "arrow-down",
        "ArrowLeft" => "arrow-left",
        "ArrowRight" => "arrow-right",
        other => other, // single chars and lowercase letters pass through
    }
}

pub fn run_real<C: Cliclick>(
    engine: &Engine,
    cmd: &harness_protocol::Command,
    cliclick: &C,
) -> InjectorResult {
    let screens = engine.screens.clone();
    let to_phys = |x: f64, y: f64| -> Result<(i32, i32), InjectorResult> {
        logical_to_physical(LogicalPoint::new(x, y), &screens)
            .ok_or_else(|| InjectorResult::Failed(format!("InvalidArguments:offscreen({x},{y})")))
    };

    use harness_protocol::Command as Cmd;
    match cmd {
        Cmd::Drag {
            from,
            to,
            modifiers,
            ..
        } => {
            let f = match to_phys(from.x, from.y) {
                Ok(v) => v,
                Err(e) => return e,
            };
            let t = match to_phys(to.x, to.y) {
                Ok(v) => v,
                Err(e) => return e,
            };
            let mods = parse_all(modifiers);
            let mut args: Vec<String> = mods
                .iter()
                .map(|m| format!("kd:{}", cliclick_name(*m)))
                .collect();
            args.extend(drag_args(f, t));
            for m in &mods {
                args.push(format!("ku:{}", cliclick_name(*m)));
            }
            let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            match cliclick.run(&arg_refs) {
                Ok(_) => InjectorResult::Ok,
                Err(e) => InjectorResult::Failed(format!("InjectorFailure:{e}")),
            }
        }
        Cmd::KeyPress { key, modifiers, .. } => {
            let mods = parse_all(modifiers);
            let mut args: Vec<String> = mods
                .iter()
                .map(|m| format!("kd:{}", cliclick_name(*m)))
                .collect();
            args.extend(key_press_args(key));
            for m in &mods {
                args.push(format!("ku:{}", cliclick_name(*m)));
            }
            let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            match cliclick.run(&arg_refs) {
                Ok(_) => InjectorResult::Ok,
                Err(e) => InjectorResult::Failed(format!("InjectorFailure:{e}")),
            }
        }
        Cmd::Save { .. } => {
            let args = key_press_args("Enter");
            let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            match cliclick.run(&arg_refs) {
                Ok(_) => InjectorResult::Ok,
                Err(e) => InjectorResult::Failed(format!("InjectorFailure:{e}")),
            }
        }
        Cmd::Cancel { .. } => {
            let args = key_press_args("Escape");
            let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            match cliclick.run(&arg_refs) {
                Ok(_) => InjectorResult::Ok,
                Err(e) => InjectorResult::Failed(format!("InjectorFailure:{e}")),
            }
        }
        _ => InjectorResult::Failed("Unsupported:real_tier_does_not_handle_this_command".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct MockCliclick {
        calls: RefCell<Vec<Vec<String>>>,
    }
    impl Cliclick for MockCliclick {
        fn run(&self, args: &[&str]) -> std::io::Result<()> {
            self.calls
                .borrow_mut()
                .push(args.iter().map(|s| s.to_string()).collect());
            Ok(())
        }
    }

    fn engine_with_one_screen() -> Engine {
        use crate::core::types::{Color, Rect, ScreenInfo};
        let mut e = Engine::new(
            "/tmp/x.png".into(),
            "png".into(),
            90,
            Color::new(255, 0, 0, 255),
            3.0,
            8.0,
        );
        e.screens = vec![ScreenInfo {
            id: "1".into(),
            name: "Main".into(),
            logical_bounds: Rect::new(0.0, 0.0, 1920.0, 1080.0),
            dpi_scale: 1.0,
            physical_origin: (0, 0),
            is_primary: true,
        }];
        e
    }

    #[test]
    fn drag_args_produces_dd_du_pair() {
        let args = drag_args((10, 20), (30, 40));
        assert_eq!(args, vec!["dd:10,20", "du:30,40"]);
    }

    #[test]
    fn key_press_args_maps_named_keys() {
        assert_eq!(key_press_args("Escape"), vec!["kp:esc"]);
        assert_eq!(key_press_args("Enter"), vec!["kp:return"]);
    }

    #[test]
    fn real_drag_invokes_cliclick_dd_du() {
        use harness_protocol::{Command as Cmd, DragPoint, MouseButton, Tier};
        let engine = engine_with_one_screen();
        let mock = MockCliclick::default();
        let cmd = Cmd::Drag {
            seq: 1,
            from: DragPoint { x: 10.0, y: 20.0 },
            to: DragPoint { x: 110.0, y: 120.0 },
            button: MouseButton::Left,
            modifiers: vec![],
            mode: Tier::Real,
        };
        let res = run_real(&engine, &cmd, &mock);
        assert!(matches!(res, InjectorResult::Ok));
        let calls = mock.calls.borrow();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].iter().any(|s| s.starts_with("dd:")));
        assert!(calls[0].iter().any(|s| s.starts_with("du:")));
    }
}
