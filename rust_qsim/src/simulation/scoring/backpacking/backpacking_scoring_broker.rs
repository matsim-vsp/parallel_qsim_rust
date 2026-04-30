use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{Receiver, Sender};
use crate::simulation::framework_events::{MobsimEvent, MobsimEventsManager, MobsimListenerRegisterFn, RuntimeEvent};
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

    data_collector: Arc<Mutex<BackpackingDataCollector>>,
}

impl BackpackingMessageBroker
{
    pub fn new(
        events_manager: Rc<RefCell<MobsimEventsManager>>,
        receiver: Receiver<InternalScoringMessage>,
        senders: Vec<Sender<InternalScoringMessage>>,
        rank: u32,
        data_collector: Arc<Mutex<BackpackingDataCollector>>
    ) -> Arc<Mutex<Self>> {
        let message_broker = Arc::new(Mutex::new(Self {
            receiver,
            senders,
            rank,
            data_collector
        }));
        Self::register_fn(Arc::clone(&message_broker))(&mut *events_manager.borrow_mut());
        message_broker
    }

    fn register_fn(scoring_broker: Arc<Mutex<BackpackingMessageBroker>>) -> Box<MobsimListenerRegisterFn> {
        Box::new(move |events: &mut MobsimEventsManager| {
            let bsb = Arc::clone(&scoring_broker);
            events.on_event(move |e: &RuntimeEvent<MobsimEvent>| {
                match e.payload {
                    MobsimEvent::AfterSimStep(_) => {
                        bsb.lock().unwrap().recv();
                    }
                    _ => {}
                }
            });
        })
    }

    pub(crate) fn send_leaving_vehicle(&mut self, target: u32, vehicle_id: Id<InternalVehicle>, passengers: HashSet<Id<InternalPerson>>) {
        let msg = InternalScoringMessage {
            from_process: self.rank,
            to_process: target,
            message: Box::new(VehicleMessage { vehicle_id, passengers }),
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

    pub(crate) fn send_leaving_backpacks(&mut self, target: u32, backpacks: HashMap<Id<InternalPerson>, Backpack>) {
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

    fn recv(&mut self) {
        while let Ok(received_msg) = self.receiver.try_recv() {
            let boxed_any = received_msg.message.into_any();

            match () {
                _ if boxed_any.is::<BackpackingMessage>() => {
                    let m = boxed_any.downcast::<BackpackingMessage>().unwrap();
                    self.data_collector.lock().unwrap().add_arriving_backpacks(m.backpacks);
                }
                _ if boxed_any.is::<VehicleMessage>() => {
                    let m = boxed_any.downcast::<VehicleMessage>().unwrap();
                    self.data_collector.lock().unwrap().add_arriving_vehicle(m.vehicle_id, m.passengers);
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

pub struct VehicleMessage {
    vehicle_id: Id<InternalVehicle>,
    passengers: HashSet<Id<InternalPerson>>
}