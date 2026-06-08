use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, Weak};
use std::sync::mpsc::{Receiver, Sender};
use nohash_hasher::IntSet;
use crate::simulation::events::EventTrait;
use crate::simulation::framework_events::QSimId;
use crate::simulation::id::Id;
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scenario::vehicles::InternalVehicle;
use crate::simulation::scoring::backpacking::backpacking_message_broker::{BackpackingMessage, FinishMessage, VehicleMessage};
use crate::simulation::scoring::homesending::homesending_data_collector::HomeSendingDataCollector;
use crate::simulation::scoring::InternalScoringMessage;

pub struct HomeSendingMessageBroker
{
    receiver: Receiver<InternalScoringMessage>,
    senders: Vec<Sender<InternalScoringMessage>>,
    neighbours: IntSet<QSimId>,
    rank: QSimId,
    
    buffer_events: HashMap<QSimId, HashMap<Id<InternalPerson>, Box<dyn EventTrait>>>,
    buffer_vehicles: HashMap<QSimId, HashMap<Id<InternalVehicle>, HashSet<Id<InternalPerson>>>>,
    data_collector: Weak<Mutex<HomeSendingDataCollector>>,
}

impl HomeSendingMessageBroker {
    pub(crate) fn add_leaving_vehicle(&mut self, target: QSimId, vehicle_id: Id<InternalVehicle>, passengers: HashSet<Id<InternalPerson>>) {
        self.buffer_vehicles.entry(target).or_insert_with(|| HashMap::new()).insert(vehicle_id, passengers);
    }
    
    pub(crate) fn add_leaving_event(&mut self, target: QSimId, person_id: Id<InternalPerson>, event: Box<dyn EventTrait>) {
        self.buffer_events.entry(target).or_insert_with(|| HashMap::new()).insert(person_id, event);
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

        for (target, events) in self.buffer_events.drain() {
            let msg = InternalScoringMessage {
                from_process: self.rank,
                to_process: target,
                message: Box::new(EventMessage { events }),
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
                _ if boxed_any.is::<EventMessage>() => {
                    let m = boxed_any.downcast::<EventMessage>().unwrap();
                    self.data_collector.upgrade().unwrap().lock().unwrap().add_arriving_events(m.events);
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



pub struct EventMessage {
    events: HashMap<Id<InternalPerson>, Box<dyn EventTrait>>,
}