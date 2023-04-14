use std::time::{Duration, Instant};
use tracing::trace;

pub fn measure_duration_and_trace<Out, F: FnOnce() -> Out>(now: u32, key: &str, f: F) -> Out {
    let start = Instant::now();
    let res = f();
    let duration = start.elapsed();
    if now % 1200 == 0 {
        trace_time_with_key(now, key, duration);
    }
    res
}

fn trace_time_with_key(now: u32, key: &str, duration: Duration) {
    trace!(now = now, key = key, duration_in_ms = duration.as_millis(),);
}
