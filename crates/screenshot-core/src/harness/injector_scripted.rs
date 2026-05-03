use crate::core::editor::Tool;
use crate::core::engine::{Engine, EngineState};
use crate::core::types::LogicalPoint;
use crate::harness::modifiers;
use harness_protocol::Command;

#[derive(Debug, Clone, PartialEq)]
pub enum InjectorResult {
    Ok,
    Failed(String),
}

pub fn run_scripted(engine: &mut Engine, cmd: &Command) -> InjectorResult {
    match cmd {
        Command::MouseMove { x, y, .. } => {
            engine.on_mouse_move(LogicalPoint::new(*x, *y));
            InjectorResult::Ok
        }
        Command::MouseDown { x, y, .. } => {
            let pt = LogicalPoint::new(*x, *y);
            match engine.state {
                EngineState::OverlayRunning => {
                    engine.on_mouse_down(pt);
                    InjectorResult::Ok
                }
                EngineState::Editing => {
                    engine.on_edit_mouse_down(pt);
                    InjectorResult::Ok
                }
                _ => InjectorResult::Failed(format!("NotAllowedInState:{}", engine.state_label())),
            }
        }
        Command::MouseUp { x, y, .. } => {
            let pt = LogicalPoint::new(*x, *y);
            match engine.state {
                EngineState::OverlayRunning | EngineState::FreeSelecting { .. } => {
                    engine.on_mouse_up(pt);
                    InjectorResult::Ok
                }
                EngineState::Editing => {
                    engine.on_edit_mouse_up(pt);
                    InjectorResult::Ok
                }
                _ => InjectorResult::Failed(format!("NotAllowedInState:{}", engine.state_label())),
            }
        }
        Command::Drag { from, to, .. } => {
            let from_pt = LogicalPoint::new(from.x, from.y);
            let to_pt = LogicalPoint::new(to.x, to.y);
            match engine.state {
                EngineState::OverlayRunning => {
                    engine.on_mouse_down(from_pt);
                    engine.on_mouse_drag(to_pt);
                    engine.on_mouse_up(to_pt);
                    InjectorResult::Ok
                }
                EngineState::Editing => {
                    engine.on_edit_mouse_down(from_pt);
                    engine.on_edit_mouse_drag(to_pt);
                    engine.on_edit_mouse_up(to_pt);
                    InjectorResult::Ok
                }
                _ => InjectorResult::Failed(format!("NotAllowedInState:{}", engine.state_label())),
            }
        }
        Command::Save { .. } => {
            engine.save();
            InjectorResult::Ok
        }
        Command::Cancel { .. } => {
            engine.cancel();
            InjectorResult::Ok
        }
        Command::KeyPress {
            key,
            modifiers: mods,
            ..
        } => {
            let m = modifiers::parse(mods);
            match (key.as_str(), m.cmd, m.shift) {
                ("Escape", _, _) => {
                    engine.cancel();
                    InjectorResult::Ok
                }
                ("Return", _, _) | ("Enter", _, _) => {
                    engine.save();
                    InjectorResult::Ok
                }
                ("z" | "Z", true, false) => {
                    engine.editor.undo();
                    InjectorResult::Ok
                }
                ("z" | "Z", true, true) => {
                    engine.editor.redo();
                    InjectorResult::Ok
                }
                ("c" | "C", _, _) if matches!(engine.state, EngineState::Editing) => {
                    engine.copy_to_clipboard();
                    InjectorResult::Ok
                }
                _ => InjectorResult::Failed(format!("Unsupported:key={}", key)),
            }
        }
        Command::TextInput { text, .. } => {
            if !matches!(engine.state, EngineState::Editing) {
                return InjectorResult::Failed("NotAllowedInState:not_editing".into());
            }
            if engine.editor.active_tool != Tool::Text {
                return InjectorResult::Failed("NotAllowedInState:tool!=Text".into());
            }
            engine.append_text_layer(text);
            InjectorResult::Ok
        }
        Command::ToolSet { tool, .. } => match Tool::from_name(tool) {
            Some(t) => {
                engine.set_active_tool(t);
                InjectorResult::Ok
            }
            None => InjectorResult::Failed(format!("InvalidArguments:tool={}", tool)),
        },
        Command::SnapshotRequest { .. } => InjectorResult::Ok,
        Command::CompositeRequest {
            save_path, format, ..
        } => {
            match crate::overlay::save::composite_and_save(
                &engine.frames,
                &engine.editor,
                save_path,
                format,
                90,
            ) {
                Ok(_) => InjectorResult::Ok,
                Err(e) => InjectorResult::Failed(format!("InjectorFailure:{}", e)),
            }
        }
        Command::WindowEnumRefresh { .. } => {
            use crate::platform::traits::PlatformWindowEnumerator;
            let windows = crate::platform::Backend::enumerate_windows();
            engine.detector = Some(crate::core::window::WindowDetector::new(windows));
            InjectorResult::Ok
        }
        Command::Hello { .. } | Command::Shutdown { .. } => InjectorResult::Ok,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::capture::MockCapture;
    use crate::core::engine::{Engine, EngineState};
    use crate::core::types::{Color, Rect};
    use harness_protocol::{Command, DragPoint, MouseButton, Tier};

    fn fresh_engine() -> Engine {
        let mut e = Engine::new(
            "/tmp/x.png".into(),
            "png".into(),
            90,
            Color::new(255, 0, 0, 255),
            3.0,
            8.0,
        );
        let cap = MockCapture::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        e.start(&cap);
        e
    }

    #[test]
    fn mouse_drag_in_scripted_yields_region_selected() {
        let mut engine = fresh_engine();
        let cmd = Command::Drag {
            seq: 10,
            from: DragPoint { x: 100.0, y: 100.0 },
            to: DragPoint { x: 300.0, y: 300.0 },
            button: MouseButton::Left,
            modifiers: vec![],
            mode: Tier::Scripted,
        };
        let result = run_scripted(&mut engine, &cmd);
        assert!(matches!(result, InjectorResult::Ok));
        assert!(matches!(engine.state, EngineState::Editing));
    }

    #[test]
    fn key_press_escape_cancels_engine() {
        let mut engine = fresh_engine();
        let cmd = Command::KeyPress {
            seq: 11,
            key: "Escape".into(),
            modifiers: vec![],
            mode: Tier::Scripted,
        };
        let result = run_scripted(&mut engine, &cmd);
        assert!(matches!(result, InjectorResult::Ok));
        assert!(engine.should_close);
    }

    #[test]
    fn tool_set_changes_active_tool() {
        let mut engine = fresh_engine();
        engine.state = EngineState::Editing;
        engine.editor.selection = Some(Rect::new(0.0, 0.0, 100.0, 100.0));
        let cmd = Command::ToolSet {
            seq: 12,
            tool: "Rect".into(),
            mode: Tier::Scripted,
        };
        let result = run_scripted(&mut engine, &cmd);
        assert!(matches!(result, InjectorResult::Ok));
        assert_eq!(engine.editor.active_tool, crate::core::editor::Tool::Rect);
    }

    #[test]
    fn text_input_outside_text_tool_returns_not_allowed() {
        let mut engine = fresh_engine();
        engine.state = EngineState::Editing;
        engine.editor.active_tool = crate::core::editor::Tool::Rect;
        let cmd = Command::TextInput {
            seq: 13,
            text: "hi".into(),
            mode: Tier::Scripted,
        };
        let result = run_scripted(&mut engine, &cmd);
        assert!(
            matches!(result, InjectorResult::Failed(reason) if reason.starts_with("NotAllowedInState"))
        );
    }
}
