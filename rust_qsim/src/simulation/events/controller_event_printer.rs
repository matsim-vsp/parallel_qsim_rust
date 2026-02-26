use crate::simulation::framework_events::{
    EventOrigin, MobsimEvent, MobsimEventsManager, MobsimListenerRegisterFn,
};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct BeforeSimStepMessage {
    pub partition: u32,
    pub iteration: u32,
    pub seq_no: u64,
    pub time: u32,
}

pub struct ControllerEventPrinter;

impl ControllerEventPrinter {
    pub fn register_fn(
        sender: mpsc::Sender<BeforeSimStepMessage>,
    ) -> Box<MobsimListenerRegisterFn> {
        Box::new(move |events: &mut MobsimEventsManager| {
            events.on_event(move |runtime_event| {
                if let MobsimEvent::BeforeSimStep(event) = &runtime_event.payload {
                    thread::sleep(Duration::from_millis(1));
                    let partition = match runtime_event.meta.origin {
                        EventOrigin::Partition(rank) => rank,
                        EventOrigin::Controller => 0,
                    };
                    let _ = sender.send(BeforeSimStepMessage {
                        partition,
                        iteration: runtime_event.meta.iteration,
                        seq_no: runtime_event.meta.seq_no,
                        time: event.time,
                    });
                }
            });
        })
    }
}
