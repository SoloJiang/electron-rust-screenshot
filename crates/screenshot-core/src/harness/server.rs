use crate::harness::transport::Transport;
use harness_protocol::{Command, CommandAck, ServerMessage, StateSnapshot};
use std::collections::VecDeque;
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

/// Live JSONL harness — connected to a real socket.
pub struct HarnessServer {
    transport: Transport,
    next_seq: u64,
    pending_commands: VecDeque<Command>,
    protocol_errors: Vec<String>,
}

impl HarnessServer {
    pub fn new(transport: Transport) -> Self {
        Self {
            transport,
            next_seq: 0,
            pending_commands: VecDeque::new(),
            protocol_errors: Vec::new(),
        }
    }

    /// Drain any newly-arrived inbound lines into the pending command queue.
    /// Non-blocking; safe to call from winit::about_to_wait.
    pub fn poll_inbound(&mut self) -> io::Result<()> {
        loop {
            match self.transport.try_read_line()? {
                Some(line) => self.ingest_line(line),
                None => return Ok(()),
            }
        }
    }

    fn ingest_line(&mut self, line: String) {
        match serde_json::from_str::<Command>(&line) {
            Ok(cmd) => self.pending_commands.push_back(cmd),
            Err(e) => self.protocol_errors.push(format!("{}: {}", e, line)),
        }
    }

    pub fn pop_command(&mut self) -> Option<Command> {
        self.pending_commands.pop_front()
    }

    pub fn drain_commands(&mut self) -> Vec<Command> {
        self.pending_commands.drain(..).collect()
    }

    pub fn drain_protocol_errors(&mut self) -> Vec<String> {
        std::mem::take(&mut self.protocol_errors)
    }

    pub fn send_hello(&mut self, version: u32, platform: String) -> io::Result<()> {
        let seq = self.next_seq();
        let msg = ServerMessage::Hello {
            ts: now_ms(),
            seq,
            version,
            platform,
        };
        self.write_message(&msg)
    }

    pub fn send_engine_event(&mut self, payload: serde_json::Value) -> io::Result<()> {
        let seq = self.next_seq();
        let msg = ServerMessage::EngineEvent {
            ts: now_ms(),
            seq,
            payload,
        };
        self.write_message(&msg)
    }

    pub fn send_state_snapshot(&mut self, snap: StateSnapshot) -> io::Result<()> {
        let seq = self.next_seq();
        let msg = ServerMessage::StateSnapshot {
            ts: now_ms(),
            seq,
            payload: snap,
        };
        self.write_message(&msg)
    }

    pub fn send_ack(&mut self, ref_seq: u64, ok: bool, error: Option<String>) -> io::Result<()> {
        let seq = self.next_seq();
        let msg = ServerMessage::CommandAck(CommandAck {
            ts: now_ms(),
            seq,
            ref_seq,
            ok,
            error,
        });
        self.write_message(&msg)
    }

    fn next_seq(&mut self) -> u64 {
        self.next_seq += 1;
        self.next_seq
    }

    fn write_message(&self, msg: &ServerMessage) -> io::Result<()> {
        let line = serde_json::to_string(msg)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        self.transport.write_line(&line)
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Test double — keeps lines in memory instead of a real socket.
#[cfg(test)]
pub(crate) struct BufferedServer {
    next_seq: u64,
    inbound: VecDeque<String>,
    outbound: Vec<String>,
    pending_commands: VecDeque<Command>,
    protocol_errors: Vec<String>,
}

#[cfg(test)]
impl BufferedServer {
    pub fn new_for_test() -> Self {
        Self {
            next_seq: 0,
            inbound: VecDeque::new(),
            outbound: Vec::new(),
            pending_commands: VecDeque::new(),
            protocol_errors: Vec::new(),
        }
    }

    pub fn feed_inbound(&mut self, line: &str) {
        self.inbound.push_back(line.to_string());
        self.flush_inbound();
    }

    fn flush_inbound(&mut self) {
        while let Some(line) = self.inbound.pop_front() {
            match serde_json::from_str::<Command>(&line) {
                Ok(cmd) => self.pending_commands.push_back(cmd),
                Err(e) => self.protocol_errors.push(format!("{}: {}", e, line)),
            }
        }
    }

    pub fn drain_commands(&mut self) -> Vec<Command> {
        self.pending_commands.drain(..).collect()
    }

    pub fn drain_protocol_errors(&mut self) -> Vec<String> {
        std::mem::take(&mut self.protocol_errors)
    }

    pub fn take_outbound(&mut self) -> Vec<String> {
        std::mem::take(&mut self.outbound)
    }

    pub fn send_engine_event(&mut self, payload: serde_json::Value) {
        self.next_seq += 1;
        let msg = ServerMessage::EngineEvent {
            ts: 0,
            seq: self.next_seq,
            payload,
        };
        self.outbound.push(serde_json::to_string(&msg).unwrap());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harness_protocol::{Command, ServerMessage, Tier};

    #[test]
    fn encode_engine_event_emits_jsonl_with_seq() {
        let mut buffered = BufferedServer::new_for_test();
        buffered.send_engine_event(serde_json::json!({"type": "cancelled"}));
        let lines = buffered.take_outbound();
        assert_eq!(lines.len(), 1);
        let parsed: ServerMessage = serde_json::from_str(&lines[0]).unwrap();
        match parsed {
            ServerMessage::EngineEvent { seq, payload, .. } => {
                assert_eq!(seq, 1);
                assert_eq!(payload.get("type").unwrap(), "cancelled");
            }
            _ => panic!("wrong variant"),
        }
        // second send increments seq
        buffered.send_engine_event(serde_json::json!({"type": "cancelled"}));
        let lines = buffered.take_outbound();
        let parsed: ServerMessage = serde_json::from_str(&lines[0]).unwrap();
        if let ServerMessage::EngineEvent { seq, .. } = parsed {
            assert_eq!(seq, 2);
        } else {
            panic!();
        }
    }

    #[test]
    fn decode_command_queues_in_order() {
        let mut buffered = BufferedServer::new_for_test();
        buffered.feed_inbound(r#"{"type":"mouse_move","seq":1,"x":1.0,"y":2.0,"mode":"scripted"}"#);
        buffered.feed_inbound(
            r#"{"type":"drag","seq":2,"from":{"x":3,"y":4},"to":{"x":5,"y":6},"button":"left","modifiers":[],"mode":"real"}"#,
        );
        let q = buffered.drain_commands();
        assert_eq!(q.len(), 2);
        match &q[0] {
            Command::MouseMove { seq, x, y, .. } => {
                assert_eq!(*seq, 1);
                assert_eq!(*x, 1.0);
                assert_eq!(*y, 2.0);
            }
            _ => panic!("wrong variant"),
        }
        match &q[1] {
            Command::Drag { mode, .. } => {
                assert_eq!(*mode, Tier::Real);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn malformed_inbound_yields_protocol_error() {
        let mut buffered = BufferedServer::new_for_test();
        buffered.feed_inbound("not-json");
        let err = buffered.drain_protocol_errors();
        assert_eq!(err.len(), 1);
        assert!(err[0].contains("not-json"));
    }
}
