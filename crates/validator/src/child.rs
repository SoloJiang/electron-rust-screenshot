use anyhow::{anyhow, Context, Result};
use interprocess::local_socket::{prelude::*, GenericFilePath, ListenerOptions, ToFsName};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

pub struct EngineChild {
    pub process: Child,
    pub socket_path: String,
    pub stream: interprocess::local_socket::Stream,
}

impl EngineChild {
    pub fn spawn(setup_json: &str, engine_path: Option<&str>) -> Result<Self> {
        let socket_path = unique_socket_path();
        let name = socket_path
            .clone()
            .to_fs_name::<GenericFilePath>()
            .context("invalid socket name")?;
        let listener = ListenerOptions::new()
            .name(name)
            .create_sync()
            .context("failed to create listener")?;

        let engine = engine_path.unwrap_or("./dist/lib");
        let script = format!(
            "const {{start}} = require('{engine}'); \
             const cfg = {setup_json}; \
             try {{ \
               const r = start(cfg); \
               console.log('engine result', JSON.stringify(r)); \
             }} catch (e) {{ \
               console.error('engine error', e && e.message); \
               process.exit(1); \
             }}"
        );

        let mut process = Command::new("node")
            .args(["-e", &script])
            .env("SCREENSHOT_HARNESS_SOCKET", &socket_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("failed to spawn node engine")?;

        let stream = accept_with_timeout(listener, &mut process)?;

        Ok(Self {
            process,
            socket_path,
            stream,
        })
    }

    pub fn shutdown(mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

impl Drop for EngineChild {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

fn accept_with_timeout(
    listener: interprocess::local_socket::Listener,
    process: &mut Child,
) -> Result<interprocess::local_socket::Stream> {
    let (tx, rx) = std::sync::mpsc::channel::<Result<interprocess::local_socket::Stream>>();
    std::thread::spawn(move || {
        if let Ok(stream) = listener.accept() {
            let _ = tx.send(Ok(stream));
        }
    });

    match rx.recv_timeout(CONNECT_TIMEOUT) {
        Ok(Ok(stream)) => Ok(stream),
        Ok(Err(e)) => Err(anyhow!("accept failed: {e}")),
        Err(_) => {
            let _ = process.kill();
            Err(anyhow!(
                "engine child did not connect within {:?}",
                CONNECT_TIMEOUT
            ))
        }
    }
}

fn unique_socket_path() -> String {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path: PathBuf = std::env::temp_dir().join(format!("validator-{pid}-{nanos}.sock"));
    path.to_string_lossy().into_owned()
}
