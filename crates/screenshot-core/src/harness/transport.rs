use interprocess::local_socket::{traits::Stream as _, GenericFilePath, Stream, ToFsName};
use interprocess::TryClone;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// One-shot bidirectional JSONL transport.
///
/// Reads are non-blocking once connected: `try_read_line` returns Ok(None)
/// when no full line is available without blocking.
pub struct Transport {
    reader: BufReader<Stream>,
    writer: Arc<Mutex<Stream>>,
    read_buffer: String,
}

impl Transport {
    pub fn connect(path: &Path) -> io::Result<Self> {
        let name = path
            .to_fs_name::<GenericFilePath>()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let conn = Stream::connect(name)?;
        let writer_clone = conn.try_clone()?;
        Self::set_nonblocking(&conn, true)?;
        Ok(Self {
            reader: BufReader::new(conn),
            writer: Arc::new(Mutex::new(writer_clone)),
            read_buffer: String::new(),
        })
    }

    pub fn from_stream(stream: Stream) -> io::Result<Self> {
        let writer_clone = stream.try_clone()?;
        Self::set_nonblocking(&stream, true)?;
        Ok(Self {
            reader: BufReader::new(stream),
            writer: Arc::new(Mutex::new(writer_clone)),
            read_buffer: String::new(),
        })
    }

    fn set_nonblocking(stream: &Stream, nb: bool) -> io::Result<()> {
        stream.set_nonblocking(nb)
    }

    /// Returns Ok(Some(line)) if a full \n-terminated line is buffered,
    /// Ok(None) if not enough data without blocking, Err on EOF or io error.
    pub fn try_read_line(&mut self) -> io::Result<Option<String>> {
        match self.reader.read_line(&mut self.read_buffer) {
            Ok(0) => {
                if self.read_buffer.is_empty() {
                    Err(io::Error::new(io::ErrorKind::UnexpectedEof, "peer closed"))
                } else {
                    // EOF with partial line remaining — return what we have
                    let line = std::mem::take(&mut self.read_buffer);
                    Ok(Some(line))
                }
            }
            Ok(_) => {
                if self.read_buffer.ends_with('\n') {
                    self.read_buffer.pop();
                    if self.read_buffer.ends_with('\r') {
                        self.read_buffer.pop();
                    }
                    Ok(Some(std::mem::take(&mut self.read_buffer)))
                } else {
                    Ok(None)
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn write_line(&self, line: &str) -> io::Result<()> {
        let mut w = self.writer.lock().unwrap();
        w.write_all(line.as_bytes())?;
        w.write_all(b"\n")?;
        w.flush()
    }

    pub fn writer_handle(&self) -> Arc<Mutex<Stream>> {
        Arc::clone(&self.writer)
    }
}
