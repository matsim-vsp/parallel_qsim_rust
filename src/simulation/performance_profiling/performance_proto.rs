use crate::simulation::performance_profiling::proto::{Metadata, ProfilingEvent};
use std::env;

use base64::Engine;
use prost::Message;
use std::time::Instant;
use tracing::trace;

const DEFAULT_PERFORMANCE_INTERVAL: u32 = 1200;

pub fn measure_duration<Out, F: FnOnce() -> Out>(
    now: Option<u32>,
    key: &str,
    metadata: Option<Metadata>,
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
        let mut buffer: Vec<u8> = Vec::new();
        ProfilingEvent::new(String::from(key), now, duration.as_secs_f64(), metadata)
            .encode(&mut buffer)
            .expect("Failed to encode ProfilingEvent");
        let event_string = base64::engine::general_purpose::STANDARD_NO_PAD.encode(buffer);
        trace!(event = event_string)
    }
    res
}

#[cfg(test)]
mod tests {
    use crate::simulation::performance_profiling::proto::metadata::Type::NodeInformation;
    use crate::simulation::performance_profiling::proto::{NodeInformationData, ProfilingEvent};
    use base64::Engine;
    use prost::Message;

    #[test]
    fn decode_base64_event() {
        let buffer = base64::engine::general_purpose::STANDARD_NO_PAD
            .decode("CgpzaW11bGF0aW9uEZSLTwbp9/o/IgsaCQgTEAEYASIBAQ")
            .expect("Something went wrong when decoding event.");

        let event = ProfilingEvent::decode(buffer.as_slice()).unwrap();

        assert_eq!(event.key, "simulation");
        assert_eq!(event.sim_time, None);
        assert_eq!(
            event.metadata.as_ref().unwrap().r#type.as_ref().unwrap(),
            &NodeInformation(NodeInformationData {
                local_links: 19,
                split_in_links: 1,
                split_out_links: 1,
                neighbours: vec![1],
            })
        );
        assert_eq!(event.duration, 1.6855249639999998);
    }
}
