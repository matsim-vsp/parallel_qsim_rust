use crate::simulation::framework_events::{
    ControllerEvent, ControllerEventsManager, ControllerListenerRegisterFn,
};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct BeforeMobsimMessage {
    pub iteration: u32,
    pub seq_no: u64,
    pub last_iteration: bool,
}

pub struct ControllerEventPrinter;

impl ControllerEventPrinter {
    pub fn register_fn(
        sender: mpsc::Sender<BeforeMobsimMessage>,
    ) -> Box<ControllerListenerRegisterFn> {
        Box::new(move |events: &mut ControllerEventsManager| {
            events.on_event(move |runtime_event| {
                if let ControllerEvent::BeforeMobsim(event) = &runtime_event.payload {
                    thread::sleep(Duration::from_secs(10));
                    let _ = sender.send(BeforeMobsimMessage {
                        iteration: runtime_event.meta.iteration,
                        seq_no: runtime_event.meta.seq_no,
                        last_iteration: event.last_iteration,
                    });
                }
            });
        })
    }

    // pub fn print_from_channel(receiver: mpsc::Receiver<BeforeMobsimMessage>) {
    //     while let Ok(msg) = receiver.recv() {
    //         println!(
    //             "[ControllerEvent] BeforeMobsim | iteration={} seq_no={} last_iteration={}",
    //             msg.iteration, msg.seq_no, msg.last_iteration
    //         );
    //     }
    // }
}
