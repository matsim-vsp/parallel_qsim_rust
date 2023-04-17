mod performance_logger;
pub mod performance_proto;
mod profiling;

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/profiling.rs"));
}

use crate::simulation::performance_profiling::proto::Metadata;

pub trait PerformanceProfiler {
    fn measure_duration<Out, F: FnOnce() -> Out>(
        &mut self,
        now: Option<u32>,
        key: &str,
        metadata: Metadata,
        f: F,
    ) -> Out;

    fn finish(&mut self) {}
}

pub struct NoPerformanceProfiler {}

impl PerformanceProfiler for NoPerformanceProfiler {
    fn measure_duration<Out, F: FnOnce() -> Out>(
        &mut self,
        now: Option<u32>,
        key: &str,
        metadata: Metadata,
        f: F,
    ) -> Out {
        f()
    }
}
