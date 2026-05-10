use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::{Rc};
use std::sync::{Arc, Mutex, Weak};
use std::sync::mpsc::{Receiver, Sender};
use crate::simulation::framework_events::{MobsimEvent, MobsimEventsManager, MobsimListenerRegisterFn, QSimId, RuntimeEvent};
use crate::simulation::id::Id;
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scenario::vehicles::InternalVehicle;
use crate::simulation::scoring::backpacking::backpack::Backpack;
use crate::simulation::scoring::backpacking::backpacking_data_collector::BackpackingDataCollector;
use crate::simulation::scoring::{InternalScoringMessage, Message};

pub struct BackpackingMessageBroker
{
    receiver: Receiver<InternalScoringMessage>,
    senders: Vec<Sender<InternalScoringMessage>>,
    rank: u32,

    buffer: HashMap<QSimId, HashMap<Id<InternalPerson>, Backpack>>,
    data_collector: Weak<Mutex<BackpackingDataCollector>>,
}

impl BackpackingMessageBroker
{
    pub(crate) fn new(
        receiver: Receiver<InternalScoringMessage>,
        senders: Vec<Sender<InternalScoringMessage>>,
        rank: u32,
    ) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            receiver,
            senders,
            rank,
            buffer: HashMap::new(),
            data_collector: Weak::new()
        }))
    }
    
    pub(crate) fn add_sender(&mut self, sender: Sender<InternalScoringMessage>) {
        self.senders.push(sender);
    }

    pub(crate) fn finish(message_broker: &Arc<Mutex<Self>>, data_collector: Weak<Mutex<BackpackingDataCollector>>){
        message_broker.lock().unwrap().data_collector = data_collector;
    }

    pub(crate) fn register_fn(scoring_broker: Arc<Mutex<BackpackingMessageBroker>>) -> Box<MobsimListenerRegisterFn> {
        Box::new(move |events: &mut MobsimEventsManager| {
            let bsb = Arc::clone(&scoring_broker);
            events.on_event(move |e: &RuntimeEvent<MobsimEvent>| {
                match e.payload {
                    MobsimEvent::AfterSimStep(_) => {
                        bsb.lock().unwrap().send_recv();
                    }
                    _ => {}
                }
            });
        })
    }

    pub(crate) fn add_leaving_backpack(&mut self, target: QSimId, agent_id: Id<InternalPerson>, backpack: Backpack) {
        self.buffer.entry(target).or_insert_with(|| HashMap::new()).insert(agent_id, backpack);
    }

    fn send_recv(&mut self) {
        for (target, backpacks) in self.buffer.drain() {
            let msg = InternalScoringMessage {
                from_process: self.rank,
                to_process: target,
                message: Box::new(BackpackingMessage { backpacks }),
            };

            let sender = self.senders.get_mut(target as usize).unwrap();
            sender.send(msg)
                .unwrap_or_else(|e| {
                    panic!(
                        "Error while sending message to rank {} with error {}",
                        target, e
                    )
                });
        }

        while let Ok(received_msg) = self.receiver.try_recv() {
            let boxed_any = received_msg.message.into_any();

            match () {
                _ if boxed_any.is::<BackpackingMessage>() => {
                    let m = boxed_any.downcast::<BackpackingMessage>().unwrap();
                    self.data_collector.upgrade().unwrap().lock().unwrap().add_arriving_backpacks(m.backpacks);
                }
                _ => {
                    panic!("Received unknown message type!");
                }

            }
        }
    }
}

pub struct BackpackingMessage {
    backpacks: HashMap<Id<InternalPerson>, Backpack>
}
