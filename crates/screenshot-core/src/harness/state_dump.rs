use crate::core::engine::Engine;
use harness_protocol::StateSnapshot;

pub fn snapshot_now(engine: &Engine) -> StateSnapshot {
    engine.snapshot_state()
}
