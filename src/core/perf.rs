use super::events::MetricsPayload;
use std::collections::VecDeque;
use std::time::Instant;

pub struct PerformanceMonitor {
    hit_test_times: VecDeque<f64>,
    last_memory_read: Option<Instant>,
}

impl PerformanceMonitor {
    pub fn new() -> Self {
        Self {
            hit_test_times: VecDeque::with_capacity(16),
            last_memory_read: None,
        }
    }

    pub fn record_capture(&self, start: Instant) -> f64 {
        start.elapsed().as_secs_f64() * 1000.0
    }

    pub fn record_window_enum(&self, start: Instant) -> f64 {
        start.elapsed().as_secs_f64() * 1000.0
    }

    pub fn record_hit_test(&mut self, start: Instant) {
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        if self.hit_test_times.len() >= 16 {
            self.hit_test_times.pop_front();
        }
        self.hit_test_times.push_back(ms);
    }

    pub fn hit_test_p99(&self) -> f64 {
        if self.hit_test_times.is_empty() {
            return 0.0;
        }
        let mut sorted: Vec<f64> = self.hit_test_times.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((sorted.len() - 1) as f64 * 0.99) as usize;
        sorted[idx]
    }

    pub fn memory_mb(&mut self) -> f64 {
        use sysinfo::{ProcessRefreshKind, RefreshKind, System};
        let s = System::new_with_specifics(RefreshKind::new().with_processes(ProcessRefreshKind::new()));
        let pid = sysinfo::get_current_pid().expect("valid pid");
        s.process(pid)
            .map(|p| p.memory() as f64 / 1024.0 / 1024.0)
            .unwrap_or(0.0)
    }

    pub fn build_payload(
        &mut self,
        capture_ms: f64,
        window_enum_ms: f64,
        frame_time_ms: f64,
        egui_paint_ms: f64,
    ) -> MetricsPayload {
        MetricsPayload {
            capture_ms,
            window_enum_ms,
            hit_test_p99_ms: self.hit_test_p99(),
            frame_time_ms,
            egui_paint_ms,
            memory_mb: self.memory_mb(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p99_calculation() {
        let mut p = PerformanceMonitor::new();
        for i in 1..=16 {
            let start = Instant::now();
            std::thread::sleep(std::time::Duration::from_micros(i * 10));
            p.record_hit_test(start);
        }
        let p99 = p.hit_test_p99();
        assert!(p99 > 0.0);
    }
}
