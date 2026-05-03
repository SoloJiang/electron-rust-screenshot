pub mod command;
pub mod server_message;
pub mod snapshot;

pub use command::{Command, DragPoint, MouseButton, Tier};
pub use server_message::{CommandAck, ServerMessage};
pub use snapshot::StateSnapshot;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_drag_round_trip() {
        let cmd = Command::Drag {
            seq: 7,
            from: DragPoint { x: 300.0, y: 400.0 },
            to: DragPoint { x: 500.0, y: 600.0 },
            button: MouseButton::Left,
            modifiers: vec!["Shift".to_string()],
            mode: Tier::Real,
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let parsed: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, cmd);
    }

    #[test]
    fn server_message_engine_event_keeps_payload_shape() {
        let raw = r#"{"type":"engine_event","ts":1,"seq":2,"payload":{"type":"cancelled"}}"#;
        let parsed: ServerMessage = serde_json::from_str(raw).unwrap();
        match parsed {
            ServerMessage::EngineEvent { ts, seq, payload } => {
                assert_eq!(ts, 1);
                assert_eq!(seq, 2);
                assert_eq!(
                    payload.get("type").and_then(|v| v.as_str()),
                    Some("cancelled")
                );
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn command_ack_failure_round_trip() {
        let msg = ServerMessage::CommandAck(CommandAck {
            ts: 100,
            seq: 5,
            ref_seq: 3,
            ok: false,
            error: Some("NotAllowedInState:Idle".into()),
        });
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, msg);
    }
}
