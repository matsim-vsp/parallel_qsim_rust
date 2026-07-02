use crate::simulation::framework_events::{
    MobsimEvent, MobsimEventsManager, MobsimListenerRegisterFn, PartitionEvent,
    PartitionEventsManager, PartitionListenerRegisterFn, QSimId, RuntimeEvent,
};
use crate::simulation::id::Id;
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scenario::vehicles::InternalVehicle;
use crate::simulation::scoring::InternalScoringMessage;
use crate::simulation::scoring::backpacking::backpack::Backpack;
use crate::simulation::scoring::backpacking::backpacking_data_collector::BackpackingDataCollector;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex, Weak};

pub struct BackpackingMessageBroker {
    receiver: Receiver<InternalScoringMessage>,
    senders: Vec<Sender<InternalScoringMessage>>,
    rank: QSimId,

    buffer_backpacks: HashMap<QSimId, HashMap<Id<InternalPerson>, Backpack>>,
    buffer_vehicles: HashMap<QSimId, HashMap<Id<InternalVehicle>, HashSet<Id<InternalPerson>>>>,
    wait_backpacks: HashSet<Id<InternalPerson>>,
    wait_vehicles: HashSet<Id<InternalVehicle>>,
    data_collector: Weak<Mutex<BackpackingDataCollector>>,
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
            buffer_backpacks: HashMap::new(),
            buffer_vehicles: HashMap::new(),
            wait_backpacks: HashSet::new(),
            wait_vehicles: HashSet::new(),
            data_collector: Weak::new(),
        }))
    }

    pub(crate) fn attach_senders(&mut self, senders: Vec<Sender<InternalScoringMessage>>) {
        self.senders.extend(senders);
    }

    pub(crate) fn init(
        message_broker: &Arc<Mutex<Self>>,
        data_collector: Weak<Mutex<BackpackingDataCollector>>,
    ) {
        message_broker.lock().unwrap().data_collector = data_collector;
    }

    pub(crate) fn register_mobsim_fn(
        scoring_broker: Arc<Mutex<BackpackingMessageBroker>>,
    ) -> Box<MobsimListenerRegisterFn> {
        Box::new(move |events: &mut MobsimEventsManager| {
            let bmb = Arc::clone(&scoring_broker);
            events.on_event(move |e: &RuntimeEvent<MobsimEvent>| match &e.payload {
                MobsimEvent::BeforeSimStep(_) => {
                    bmb.lock().unwrap().recv_backpacks();
                    bmb.lock().unwrap().recv_vehicles();
                }
                MobsimEvent::AfterSimStep(_) => {
                    bmb.lock().unwrap().send();
                }
                _ => {}
            });
        })
    }

    pub(crate) fn register_partition_fn(
        scoring_broker: Arc<Mutex<BackpackingMessageBroker>>,
    ) -> Box<PartitionListenerRegisterFn> {
        Box::new(move |events: &mut PartitionEventsManager| {
            let bmb = Arc::clone(&scoring_broker);
            events.on_event(move |e: &RuntimeEvent<PartitionEvent>| match &e.payload {
                PartitionEvent::VehicleLeavesPartition(i) => {
                    let leaving_vehicle = bmb
                        .lock()
                        .unwrap()
                        .data_collector
                        .upgrade()
                        .unwrap()
                        .lock()
                        .unwrap()
                        .remove_leaving_vehicles(&i.vehicle_id);
                    bmb.lock().unwrap().add_leaving_vehicle(
                        i.to.clone(),
                        i.vehicle_id.clone(),
                        leaving_vehicle,
                    );
                }
                PartitionEvent::AgentLeavesPartition(i) => {
                    let leaving_backpack = bmb
                        .lock()
                        .unwrap()
                        .data_collector
                        .upgrade()
                        .unwrap()
                        .lock()
                        .unwrap()
                        .remove_leaving_backpack(&i.agent_id);
                    bmb.lock().unwrap().add_leaving_backpack(
                        i.to.clone(),
                        i.agent_id.clone(),
                        leaving_backpack,
                    );
                }
                PartitionEvent::AgentEntersPartition(i) => {
                    bmb.lock()
                        .unwrap()
                        .wait_backpacks
                        .insert(i.agent_id.clone());
                }
                PartitionEvent::VehicleEntersPartition(i) => {
                    bmb.lock()
                        .unwrap()
                        .wait_vehicles
                        .insert(i.vehicle_id.clone());
                }
            });
        })
    }

    pub(crate) fn add_leaving_backpack(
        &mut self,
        target: QSimId,
        agent_id: Id<InternalPerson>,
        backpack: Backpack,
    ) {
        self.buffer_backpacks
            .entry(target)
            .or_insert_with(|| HashMap::new())
            .insert(agent_id, backpack);
    }

    pub(crate) fn add_leaving_vehicle(
        &mut self,
        target: QSimId,
        vehicle_id: Id<InternalVehicle>,
        passengers: HashSet<Id<InternalPerson>>,
    ) {
        self.buffer_vehicles
            .entry(target)
            .or_insert_with(|| HashMap::new())
            .insert(vehicle_id, passengers);
    }

    pub(crate) fn send(&mut self) {
        for (target, vehicles) in self.buffer_vehicles.drain() {
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

        for (target, backpacks) in self.buffer_backpacks.drain() {
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
                self.data_collector
                    .upgrade()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .add_arriving_vehicles(m.vehicles);
            }
            _ if boxed_any.is::<BackpackingMessage>() => {
                let m = boxed_any.downcast::<BackpackingMessage>().unwrap();
                self.data_collector
                    .upgrade()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .add_arriving_backpacks(m.backpacks);
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
    fn recv_backpacks(&mut self) {
        let pending_backpacks = self.wait_backpacks.drain().collect::<Vec<_>>();
        for person_id in pending_backpacks {
            // Check whether backpack is already there
            if self
                .data_collector
                .upgrade()
                .unwrap()
                .lock()
                .unwrap()
                .get_backpacks()
                .contains_key(&person_id)
            {
                // Since the backpack is already there, there is no need to block the thread
                return;
            }

            // Recv backpacks, until the backpack for the current person arrives
            while !self
                .data_collector
                .upgrade()
                .unwrap()
                .lock()
                .unwrap()
                .get_backpacks()
                .contains_key(&person_id)
            {
                let received_msg = self.receiver.recv().expect("Error receiving message");
                self.recv(received_msg);
            }
        }
    }

    /// Called upon a VehicleEntersPartitionEvent. It checks whether the passenger info of arrived
    /// vehicles is present. If not, the function blocks the thread until said passenger info has arrived.
    /// This function is called once for each vehicle, that has entered the partition and assures
    /// that the scoring module is only using passenger info which is present.
    fn recv_vehicles(&mut self) {
        let pending_vehicles = self.wait_vehicles.drain().collect::<Vec<_>>();
        for vehicle_id in pending_vehicles {
            // Check whether backpack is already there
            if self
                .data_collector
                .upgrade()
                .unwrap()
                .lock()
                .unwrap()
                .get_vehicles()
                .contains_key(&vehicle_id)
            {
                // Since the backpack is already there, there is no need to block the thread
                return;
            }

            // Recv backpacks, until the backpack for the current person arrives
            while !self
                .data_collector
                .upgrade()
                .unwrap()
                .lock()
                .unwrap()
                .get_vehicles()
                .contains_key(&vehicle_id)
            {
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

        let mut finished_partitions: HashSet<QSimId> = HashSet::new();
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

// TODO Adjust access modifiers
pub struct VehicleMessage {
    pub(crate) vehicles: HashMap<Id<InternalVehicle>, HashSet<Id<InternalPerson>>>,
}

pub struct BackpackingMessage {
    backpacks: HashMap<Id<InternalPerson>, Backpack>,
}

pub struct FinishMessage {}
