use crate::simulation::framework_events::QSimId;
use crate::simulation::id::Id;
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scenario::vehicles::InternalVehicle;
use crate::simulation::scoring::InternalScoringMessage;
use crate::simulation::scoring::backpacking::backpack::Backpack;
use nohash_hasher::{IntMap, IntSet};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

pub struct BackpackingMessageBroker {
    receiver: Receiver<InternalScoringMessage>,
    senders: Vec<Sender<InternalScoringMessage>>,
    rank: QSimId,

    leaving_buffer_backpacks: IntMap<QSimId, IntMap<Id<InternalPerson>, Backpack>>,
    leaving_buffer_vehicles:
        IntMap<QSimId, IntMap<Id<InternalVehicle>, IntSet<Id<InternalPerson>>>>,
    arriving_buffer_backpacks: IntMap<Id<InternalPerson>, Backpack>,
    arriving_buffer_vehicles: IntMap<Id<InternalVehicle>, IntSet<Id<InternalPerson>>>,
    wait_backpacks: IntSet<Id<InternalPerson>>,
    wait_vehicles: IntSet<Id<InternalVehicle>>,
}

impl BackpackingMessageBroker {
    pub(crate) fn new(
        receiver: Receiver<InternalScoringMessage>,
        senders: Vec<Sender<InternalScoringMessage>>,
        rank: QSimId,
    ) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            receiver,
            senders,
            rank,
            leaving_buffer_backpacks: IntMap::default(),
            leaving_buffer_vehicles: IntMap::default(),
            arriving_buffer_backpacks: IntMap::default(),
            arriving_buffer_vehicles: IntMap::default(),
            wait_backpacks: IntSet::default(),
            wait_vehicles: IntSet::default(),
        }))
    }

    pub(crate) fn attach_senders(&mut self, senders: Vec<Sender<InternalScoringMessage>>) {
        self.senders.extend(senders);
    }

    pub(crate) fn add_leaving_backpack(
        &mut self,
        target: QSimId,
        agent_id: Id<InternalPerson>,
        backpack: Backpack,
    ) {
        self.leaving_buffer_backpacks
            .entry(target)
            .or_insert_with(|| IntMap::default())
            .insert(agent_id, backpack);
    }

    pub(crate) fn add_leaving_vehicle(
        &mut self,
        target: QSimId,
        vehicle_id: Id<InternalVehicle>,
        passengers: IntSet<Id<InternalPerson>>,
    ) {
        self.leaving_buffer_vehicles
            .entry(target)
            .or_insert_with(|| IntMap::default())
            .insert(vehicle_id, passengers);
    }

    pub(crate) fn wait_for_backpack(&mut self, person_id: Id<InternalPerson>) {
        self.wait_backpacks.insert(person_id);
    }

    pub(crate) fn wait_for_vehicle(&mut self, vehicle_id: Id<InternalVehicle>) {
        self.wait_vehicles.insert(vehicle_id);
    }

    pub(crate) fn drain_arrived_backpacks(&mut self) -> IntMap<Id<InternalPerson>, Backpack> {
        std::mem::take(&mut self.arriving_buffer_backpacks)
    }

    pub(crate) fn drain_arrived_vehicles(
        &mut self,
    ) -> IntMap<Id<InternalVehicle>, IntSet<Id<InternalPerson>>> {
        std::mem::take(&mut self.arriving_buffer_vehicles)
    }

    pub(crate) fn send(&mut self) {
        for (target, vehicles) in self.leaving_buffer_vehicles.drain() {
            let msg = InternalScoringMessage {
                from_process: self.rank,
                to_process: target,
                message: Box::new(VehicleMessage { vehicles }),
            };

            self.senders[target as usize].send(msg).unwrap_or_else(|e| {
                panic!(
                    "Error sending EventMessage to rank {} with error {}",
                    target, e
                )
            });
        }

        for (target, backpacks) in self.leaving_buffer_backpacks.drain() {
            let msg = InternalScoringMessage {
                from_process: self.rank,
                to_process: target,
                message: Box::new(BackpackingMessage { backpacks }),
            };

            self.senders[target as usize].send(msg).unwrap_or_else(|e| {
                panic!(
                    "Error sending EventMessage to rank {} with error {}",
                    target, e
                )
            });
        }
    }

    /// General receive logic for backpacking messages, called by recv_backpack() and recv_vehicle()
    fn recv(&mut self, received_msg: InternalScoringMessage) {
        let boxed_any = received_msg.message.into_any();

        match () {
            _ if boxed_any.is::<VehicleMessage>() => {
                let m = boxed_any.downcast::<VehicleMessage>().unwrap();
                self.arriving_buffer_vehicles.extend(m.vehicles);
            }
            _ if boxed_any.is::<BackpackingMessage>() => {
                let m = boxed_any.downcast::<BackpackingMessage>().unwrap();
                self.arriving_buffer_backpacks.extend(m.backpacks);
            }
            _ => {
                panic!("Received unknown message type!");
            }
        }
    }

    /// Called upon a PersonEntersPartitionEvent. It checks whether the backpacks of arrived
    /// persons are present. If not, the function blocks the thread until said backpack has arrived.
    /// This function is called once for each person, that has entered the partition and assures
    /// that the scoring module is only using backpacks which are present.
    pub(crate) fn recv_backpacks(&mut self) {
        let pending_backpacks = self.wait_backpacks.drain().collect::<Vec<_>>();
        for person_id in pending_backpacks {
            // Check whether backpack is already there
            if self.arriving_buffer_backpacks.contains_key(&person_id) {
                // Since the backpack is already there, there is no need to block the thread
                continue;
            }

            // Recv backpacks, until the backpack for the current person arrives
            while !self.arriving_buffer_backpacks.contains_key(&person_id) {
                let received_msg = self.receiver.recv().expect("Error receiving message");
                self.recv(received_msg);
            }
        }
    }

    /// Called upon a VehicleEntersPartitionEvent. It checks whether the passenger info of arrived
    /// vehicles is present. If not, the function blocks the thread until said passenger info has arrived.
    /// This function is called once for each vehicle, that has entered the partition and assures
    /// that the scoring module is only using passenger info which is present.
    pub(crate) fn recv_vehicles(&mut self) {
        let pending_vehicles = self.wait_vehicles.drain().collect::<Vec<_>>();
        for vehicle_id in pending_vehicles {
            // Check whether backpack is already there
            if self.arriving_buffer_vehicles.contains_key(&vehicle_id) {
                // Since the vehicle is already there, there is no need to block the thread
                continue;
            }

            // Recv backpacks, until the backpack for the current person arrives
            while !self.arriving_buffer_vehicles.contains_key(&vehicle_id) {
                let received_msg = self.receiver.recv().expect("Error receiving message");
                self.recv(received_msg);
            }
        }
    }

    /// The last send-recv operation before the iteration ends.
    /// Since there are no Partition Events that finalize the iteration, the certification is done
    /// manually by sending O(n^2) messages.
    pub(crate) fn finish_send_recv(&mut self) {
        self.send();

        // Send a finish message to all partitions
        for target in 0..self.senders.len() {
            if target == self.rank as usize {
                continue;
            }

            let msg = InternalScoringMessage {
                from_process: self.rank,
                to_process: target as QSimId,
                message: Box::new(FinishMessage {}),
            };

            self.senders[target].send(msg).unwrap_or_else(|e| {
                panic!(
                    "Error sending EventMessage to rank {} with error {}",
                    target, e
                )
            });
        }

        let mut finished_partitions: IntSet<QSimId> = IntSet::default();
        while finished_partitions.len() < self.senders.len() - 1 {
            let received_msg = self.receiver.recv().expect("Error receiving message");
            let boxed_any = received_msg.message.as_any();

            match () {
                _ if boxed_any.is::<FinishMessage>() => {
                    // Add finish message to set for break condition
                    finished_partitions.insert(received_msg.from_process);
                }
                _ => {
                    // Process arriving data
                    self.recv(received_msg);
                }
            }
        }
    }
}

struct VehicleMessage {
    vehicles: IntMap<Id<InternalVehicle>, IntSet<Id<InternalPerson>>>,
}

struct BackpackingMessage {
    backpacks: IntMap<Id<InternalPerson>, Backpack>,
}

struct FinishMessage {}
