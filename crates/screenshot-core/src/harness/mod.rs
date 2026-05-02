pub mod dispatch;
pub mod injector_real;
pub mod injector_scripted;
pub mod modifiers;
pub mod server;
pub mod state_dump;
pub mod transport;

use crate::harness::dispatch::DispatchState;
use crate::harness::server::HarnessServer;
use crate::harness::transport::Transport;

pub struct Harness {
    pub server: HarnessServer,
    pub dispatch_state: DispatchState,
}

pub fn bootstrap() -> Option<Harness> {
    let socket = std::env::var("SCREENSHOT_HARNESS_SOCKET").ok()?;
    if socket.is_empty() {
        return None;
    }
    match Transport::connect(std::path::Path::new(&socket)) {
        Ok(transport) => {
            let mut server = HarnessServer::new(transport);
            if let Err(e) = server.send_hello(1, format!("pid={}", std::process::id())) {
                log::warn!("harness: failed to send hello: {e}");
                return None;
            }
            Some(Harness {
                server,
                dispatch_state: DispatchState::default(),
            })
        }
        Err(e) => {
            log::warn!("harness: failed to connect to {socket}: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_returns_none_without_env() {
        std::env::remove_var("SCREENSHOT_HARNESS_SOCKET");
        assert!(bootstrap().is_none());
    }

    #[test]
    fn bootstrap_returns_none_for_unreachable_socket() {
        std::env::set_var(
            "SCREENSHOT_HARNESS_SOCKET",
            "/tmp/__nonexistent_harness_sock",
        );
        assert!(bootstrap().is_none());
        std::env::remove_var("SCREENSHOT_HARNESS_SOCKET");
    }
}
