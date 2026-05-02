use anyhow::{Context, Result};
use interprocess::local_socket::{prelude::*, GenericFilePath, ListenerOptions, ToFsName};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

pub struct EngineChild {
    pub process: Child,
    pub socket_path: String,
    pub stream: interprocess::local_socket::Stream,
}

impl EngineChild {
    pub fn spawn(setup_json: &str) -> Result<Self> {
        let socket_path = unique_socket_path();
        let name = socket_path
            .clone()
            .to_fs_name::<GenericFilePath>()
            .context("invalid socket name")?;
        let listener = ListenerOptions::new()
            .name(name)
            .create_sync()
            .context("failed to create listener")?;

        let script = format!(
            "const {{start}} = require('./dist/lib'); \
             const cfg = {setup_json}; \
             try {{ \
               const r = start(cfg); \
               console.log('engine result', JSON.stringify(r)); \
             }} catch (e) {{ \
               console.error('engine error', e && e.message); \
               process.exit(1); \
             }}"
        );

        let process = Command::new("node")
            .args(["-e", &script])
            .env("SCREENSHOT_HARNESS_SOCKET", &socket_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("failed to spawn node engine")?;

        let stream = listener.accept().context("listener.accept")?;

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

fn unique_socket_path() -> String {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path: PathBuf = std::env::temp_dir().join(format!("validator-{pid}-{nanos}.sock"));
    path.to_string_lossy().into_owned()
}
