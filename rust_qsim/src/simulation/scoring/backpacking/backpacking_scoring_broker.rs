use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, Weak};
use std::sync::mpsc::{Receiver, Sender};
use nohash_hasher::IntSet;
use crate::simulation::framework_events::{MobsimEvent, MobsimEventsManager, MobsimListenerRegisterFn, QSimId, RuntimeEvent};
use crate::simulation::id::Id;
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scenario::vehicles::InternalVehicle;
use crate::simulation::scoring::backpacking::backpack::Backpack;
use crate::simulation::scoring::backpacking::backpacking_data_collector::BackpackingDataCollector;
use crate::simulation::scoring::{InternalScoringMessage};

pub struct BackpackingMessageBroker
{
    receiver: Receiver<InternalScoringMessage>,
    senders: Vec<Sender<InternalScoringMessage>>,
    neighbours: IntSet<QSimId>,
    rank: QSimId,

    buffer_backpacks: HashMap<QSimId, HashMap<Id<InternalPerson>, Backpack>>,
    buffer_vehicles: HashMap<QSimId, HashMap<Id<InternalVehicle>, HashSet<Id<InternalPerson>>>,>,
    data_collector: Weak<Mutex<BackpackingDataCollector>>,
}

impl BackpackingMessageBroker
{
    pub(crate) fn new(
        receiver: Receiver<InternalScoringMessage>,
        senders: Vec<Sender<InternalScoringMessage>>,
        neighbours: IntSet<QSimId>,
        rank: QSimId,
    ) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            receiver,
            senders,
            neighbours,
            rank,
            buffer_backpacks: HashMap::new(),
            buffer_vehicles: HashMap::new(),
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
                match &e.payload {
                    MobsimEvent::AfterSimStep(i) => {
                        bsb.lock().unwrap().send_recv(i.time);
                    }
                    _ => {}
                }
            });
        })
    }

    pub(crate) fn add_leaving_backpack(&mut self, target: QSimId, agent_id: Id<InternalPerson>, backpack: Backpack) {
        self.buffer_backpacks.entry(target).or_insert_with(|| HashMap::new()).insert(agent_id, backpack);
    }
    
    pub(crate) fn add_leaving_vehicle(&mut self, target: QSimId, vehicle_id: Id<InternalVehicle>, passengers: HashSet<Id<InternalPerson>>) {
        self.buffer_vehicles.entry(target).or_insert_with(|| HashMap::new()).insert(vehicle_id, passengers);
    }

    fn send_recv(&mut self, now: u32) {
        for (target, vehicles) in self.buffer_vehicles.drain() {
            let msg = InternalScoringMessage {
                from_process: self.rank,
                to_process: target,
                message: Box::new(VehicleMessage { vehicles }),
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
        
        for (target, backpacks) in self.buffer_backpacks.drain() {
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

        for target in self.neighbours.iter() {
            let msg = InternalScoringMessage {
                from_process: self.rank,
                to_process: *target as QSimId,
                message: Box::new(FinishMessage{
                    time: now
                })
            };

            let sender = self.senders.get_mut(*target as usize).unwrap();
            sender.send(msg)
                .unwrap_or_else(|e| {
                    panic!(
                        "Error while sending message to rank {} with error {}",
                        target, e
                    )
                })
        }

        let mut finished_partitions: HashSet<QSimId> = HashSet::new();
        while finished_partitions.len() < self.neighbours.len() {
            let received_msg = self.receiver.recv().expect("Error receiving message");

            let boxed_any = received_msg.message.into_any();

            match () {
                _ if boxed_any.is::<VehicleMessage>() => {
                    let m = boxed_any.downcast::<VehicleMessage>().unwrap();
                    self.data_collector.upgrade().unwrap().lock().unwrap().add_arriving_vehicles(m.vehicles);
                }
                _ if boxed_any.is::<BackpackingMessage>() => {
                    let m = boxed_any.downcast::<BackpackingMessage>().unwrap();
                    self.data_collector.upgrade().unwrap().lock().unwrap().add_arriving_backpacks(m.backpacks);
                }
                _ if boxed_any.is::<FinishMessage>() => {
                    let m = boxed_any.downcast::<FinishMessage>().unwrap();
                    if m.time != now {
                        panic!("Received finish message from past or future time step!")
                    }
                    if !self.neighbours.contains(&received_msg.from_process) {
                        panic!("Received finish message from non-neighbouring partition!")
                    }
                    finished_partitions.insert(received_msg.from_process);
                }
                _ => {
                    panic!("Received unknown message type!");
                }
            }
        }
    }
}

pub struct VehicleMessage {
    vehicles: HashMap<Id<InternalVehicle>, HashSet<Id<InternalPerson>>>,
}

pub struct BackpackingMessage {
    backpacks: HashMap<Id<InternalPerson>, Backpack>
}

pub struct FinishMessage {
    time: u32
}