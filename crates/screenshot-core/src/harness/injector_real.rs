//! Real-tier injector for macOS — drives the system event queue via cliclick.

use std::path::PathBuf;
use std::process::Command;

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
        let status = Command::new(&self.path).args(args).status()?;
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
