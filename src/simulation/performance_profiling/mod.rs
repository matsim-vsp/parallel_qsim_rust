use serde_json::{json, Value};
use std::env;
use std::time::Instant;
use tracing::trace;

const DEFAULT_PERFORMANCE_INTERVAL: u32 = 900;

pub fn measure_duration<Out, F: FnOnce() -> Out>(
    now: Option<u32>,
    key: &str,
    metadata: Option<Value>,
    f: F,
) -> Out {
    let start = Instant::now();
    let res = f();
    let duration = start.elapsed();

    let interval = match env::var("RUST_Q_SIM_PERFORMANCE_TRACING_INTERVAL") {
        Ok(interval) => interval
            .parse::<u32>()
            .unwrap_or(DEFAULT_PERFORMANCE_INTERVAL),
        Err(_) => DEFAULT_PERFORMANCE_INTERVAL,
    };

    if now.map_or(true, |time| time % interval == 0) {
        let event = json!({
            "now": now,
            "key": key,
            "duration": duration,
            "metadata": metadata
        });

        trace!(event = event.to_string());
    }
    res
}
