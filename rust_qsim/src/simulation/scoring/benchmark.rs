use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;
use crate::simulation::framework_events::{MobsimEvent, MobsimEventsManager, MobsimListenerRegisterFn, QSimId, RuntimeEvent};

pub struct Benchmark {
    rank: QSimId
}

impl Benchmark {
    pub(crate) fn register_fn(self) -> Box<MobsimListenerRegisterFn> {
        Box::new(move |events: &mut MobsimEventsManager| {
            events.on_event(move |e: &RuntimeEvent<MobsimEvent>| {
                match &e.payload {
                    MobsimEvent::AfterSimStep(i) => {
                        info!(
                            target: "benchmark",
                            partition = self.rank,
                            sim_time = i.time,
                            unix_time = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_nanos(),
                            "after_sim_step"
                        );
                    }
                    _ => {}
                }
            });
        })
    }

    pub fn setup_benchmark(num_parts: u32) -> Vec<Box<MobsimListenerRegisterFn>> {
        let mut mobsim_register_fn = Vec::default();
        for n in 0..num_parts {
            let b = Benchmark {
                rank: n as QSimId
            };
            mobsim_register_fn.push(b.register_fn());
        }
        mobsim_register_fn
    }
}