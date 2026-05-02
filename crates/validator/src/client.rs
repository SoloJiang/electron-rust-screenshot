use anyhow::Result;
use harness_protocol::{Command, ServerMessage};
use interprocess::TryClone;
use std::io::{BufRead, BufReader, Write};

pub struct Client {
    reader: BufReader<interprocess::local_socket::Stream>,
    writer: interprocess::local_socket::Stream,
}

impl Client {
    pub fn new(stream: interprocess::local_socket::Stream) -> Result<Self> {
        let writer = stream.try_clone()?;
        let reader = BufReader::new(stream);
        Ok(Self { reader, writer })
    }

    pub fn send(&mut self, cmd: &Command) -> Result<()> {
        let line = serde_json::to_string(cmd)?;
        self.writer.write_all(line.as_bytes())?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;
        Ok(())
    }

    pub fn try_recv(&mut self) -> Result<Option<ServerMessage>> {
        let mut line = String::new();
        let n = self.reader.read_line(&mut line)?;
        if n == 0 {
            return Ok(None); // EOF
        }
        let msg: ServerMessage = serde_json::from_str(line.trim_end())?;
        Ok(Some(msg))
    }
}
