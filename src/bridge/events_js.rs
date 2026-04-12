use crate::core::events::EngineEvent;

pub fn serialize_event(event: &EngineEvent) -> String {
    match event {
        EngineEvent::Started { screens } => {
            let screens_json: Vec<String> = screens
                .iter()
                .map(|s| {
                    format!(
                        "{{\"id\":\"{}\",\"name\":\"{}\",\"logicalBounds\":{{\"x\":{},\"y\":{},\"w\":{},\"h\":{}}},\"dpiScale\":{}}}",
                        s.id,
                        s.name,
                        s.logical_bounds.x,
                        s.logical_bounds.y,
                        s.logical_bounds.w,
                        s.logical_bounds.h,
                        s.dpi_scale
                    )
                })
                .collect();
            format!(
                "{{\"type\":\"started\",\"screens\":[{}]}}",
                screens_json.join(",")
            )
        }
        EngineEvent::WindowHovered { window } => {
            format!(
                "{{\"type\":\"windowHovered\",\"window\":{{\"id\":\"{}\",\"title\":\"{}\",\"bounds\":{{\"x\":{},\"y\":{},\"w\":{},\"h\":{}}}}}}}",
                window.id, window.title, window.bounds.x, window.bounds.y, window.bounds.w, window.bounds.h
            )
        }
        EngineEvent::RegionSelected { screen_id, rect } => {
            format!(
                "{{\"type\":\"regionSelected\",\"screenId\":\"{}\",\"rect\":{{\"x\":{},\"y\":{},\"w\":{},\"h\":{}}}}}",
                screen_id, rect.x, rect.y, rect.w, rect.h
            )
        }
        EngineEvent::Saved { path, copied } => {
            format!(
                "{{\"type\":\"saved\",\"path\":\"{}\",\"copied\":{}}}",
                path, copied
            )
        }
        EngineEvent::Cancelled => "{\"type\":\"cancelled\"}".to_string(),
        EngineEvent::Error { code, message } => {
            format!(
                "{{\"type\":\"error\",\"code\":\"{}\",\"message\":\"{}\"}}",
                code,
                message.replace('\\', "\\\\").replace('"', "\\\"")
            )
        }
        EngineEvent::Metrics(m) => {
            format!(
                "{{\"type\":\"metrics\",\"captureMs\":{},\"windowEnumMs\":{},\"hitTestP99Ms\":{},\"frameTimeMs\":{},\"eguiPaintMs\":{},\"memoryMb\":{}}}",
                m.capture_ms, m.window_enum_ms, m.hit_test_p99_ms, m.frame_time_ms, m.egui_paint_ms, m.memory_mb
            )
        }
    }
}
