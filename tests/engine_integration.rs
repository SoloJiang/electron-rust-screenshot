use electron_rust_screenshot::core::capture::MockCapture;
use electron_rust_screenshot::core::engine::Engine;
use electron_rust_screenshot::core::events::EngineEvent;
use electron_rust_screenshot::core::types::{Color, Rect};

#[test]
fn engine_cancel_after_start() {
    let mut engine = Engine::new(
        "/tmp/test.png".into(),
        "png".into(),
        90,
        Color::new(255, 0, 0, 255),
        3.0,
        8.0,
    );
    let cap = MockCapture::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
    engine.start(&cap);
    // Consume the Started event emitted by start()
    assert!(matches!(engine.event_bus.try_recv(), Some(EngineEvent::Started { .. })));
    engine.cancel();
    assert!(matches!(engine.state, electron_rust_screenshot::core::engine::EngineState::Idle));
    let event = engine.event_bus.try_recv().unwrap();
    assert!(matches!(event, EngineEvent::Cancelled));
}
