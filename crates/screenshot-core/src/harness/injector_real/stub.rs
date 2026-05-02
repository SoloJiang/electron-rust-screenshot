//! Stub real-tier injector for non-macOS platforms.

use crate::core::engine::Engine;
use crate::harness::injector_scripted::InjectorResult;
use std::path::PathBuf;

pub trait Cliclick {
    fn run(&self, _args: &[&str]) -> std::io::Result<()> {
        Ok(())
    }
}

pub struct CliclickBin {
    pub path: PathBuf,
}

impl CliclickBin {
    pub fn discover() -> Option<Self> {
        None
    }
}

impl Cliclick for CliclickBin {
    fn run(&self, _args: &[&str]) -> std::io::Result<()> {
        Ok(())
    }
}

pub fn run_real(
    _engine: &Engine,
    _cmd: &harness_protocol::Command,
    _cliclick: &dyn Cliclick,
) -> InjectorResult {
    InjectorResult::Failed("Unsupported:real_tier_not_available_on_this_platform".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_returns_none() {
        assert!(CliclickBin::discover().is_none());
    }

    #[test]
    fn run_real_returns_unsupported() {
        let engine = Engine::new(
            "/tmp/x.png".into(),
            "png".into(),
            90,
            crate::core::types::Color::new(255, 0, 0, 255),
            3.0,
            8.0,
        );
        let mock = CliclickBin {
            path: PathBuf::from("/dev/null"),
        };
        let cmd = harness_protocol::Command::Save {
            seq: 1,
            mode: harness_protocol::Tier::Real,
        };
        let res = run_real(&engine, &cmd, &mock);
        assert!(
            matches!(res, InjectorResult::Failed(ref s) if s.contains("Unsupported")),
            "expected Unsupported error, got {:?}",
            res
        );
    }
}
