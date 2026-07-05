use crate::simulation::events::{
    ActivityEndEvent, ActivityStartEvent, EventHandlerRegisterFn, EventTrait, EventsManager,
    LinkEnterEvent, PersonArrivalEvent, PersonDepartureEvent, PersonEntersVehicleEvent,
    PersonLeavesVehicleEvent, TeleportationArrivalEvent, VehicleEntersTrafficEvent,
    VehicleLeavesTrafficEvent,
};
use crate::simulation::framework_events::{
    PartitionEvent, PartitionEventsManager, PartitionListenerRegisterFn, QSimId, RuntimeEvent,
};
use crate::simulation::id::Id;
use crate::simulation::scenario::population::{InternalPerson, Population};
use crate::simulation::scenario::vehicles::InternalVehicle;
use crate::simulation::scoring::backpacking::backpack::Backpack;
use crate::simulation::scoring::backpacking::backpacking_message_broker::BackpackingMessageBroker;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

pub struct BackpackingDataCollector {
    person_id2backpack: HashMap<Id<InternalPerson>, Backpack>,
    vehicle_id2person_ids: HashMap<Id<InternalVehicle>, HashSet<Id<InternalPerson>>>,
    rank: QSimId,

    message_broker: Arc<Mutex<BackpackingMessageBroker>>,

    // Vehicles that crossed into this partition in the current step but whose scoring mapping has
    // not arrived yet (it travels via the broker's AfterSimStep -> BeforeSimStep cycle, one step
    // behind the vehicle body). LinkEnterEvents for these vehicles are stored in
    // deferred_link_events and replayed once both the mapping and the backpack are available.
    pending_vehicles: HashSet<Id<InternalVehicle>>,
    deferred_link_events: Vec<LinkEnterEvent>,
}

impl BackpackingDataCollector {
    pub fn new(
        population: &Population,
        rank: QSimId,
        message_broker: Arc<Mutex<BackpackingMessageBroker>>,
    ) -> Arc<Mutex<Self>> {
        let data_collector = Arc::new(Mutex::new(Self {
            person_id2backpack: Default::default(),
            vehicle_id2person_ids: Default::default(),
            rank,
            message_broker,
            pending_vehicles: Default::default(),
            deferred_link_events: Default::default(),
        }));
        data_collector
            .lock()
            .unwrap()
            .generate_backpacks_for_population(&population);
        data_collector
    }

    fn generate_backpacks_for_population(&mut self, population: &Population) {
        for person in population.persons.keys() {
            self.person_id2backpack
                .insert(person.clone(), Backpack::new(person.clone(), self.rank));
        }
    }

    pub(crate) fn add_arriving_vehicles(
        &mut self,
        arriving_vehicles: HashMap<Id<InternalVehicle>, HashSet<Id<InternalPerson>>>,
    ) {
        for (vehicle_id, persons) in arriving_vehicles {
            self.pending_vehicles.remove(&vehicle_id);
            self.vehicle_id2person_ids.insert(vehicle_id, persons);
        }
    }

    /// Replays LinkEnterEvents that were buffered because the vehicle-to-person mapping had not
    /// yet arrived when they fired. Only called from recv_vehicles(), after recv_backpacks() has
    /// already run, so both backpacks and vehicle mappings are guaranteed to be present.
    pub(crate) fn replay_deferred_link_events(&mut self) {
        for event in std::mem::take(&mut self.deferred_link_events) {
            self.handle_event(&event);
        }
    }

    pub(crate) fn add_arriving_backpacks(
        &mut self,
        arriving_backpack: HashMap<Id<InternalPerson>, Backpack>,
    ) {
        self.person_id2backpack.extend(arriving_backpack);
    }

    fn remove_leaving_vehicles(
        &mut self,
        vehicle_id: &Id<InternalVehicle>,
    ) -> HashSet<Id<InternalPerson>> {
        // TODO Build a checker, so that it only allows missing entries for teleported modes
        self.vehicle_id2person_ids
            .remove(vehicle_id)
            .unwrap_or_else(|| {
                // warn!("Partition #{}: Tried to remove vehicle {}, which has no entry!", self.rank, vehicle_id);
                return HashSet::default();
            })
    }

    fn remove_leaving_backpack(&mut self, person_id: &Id<InternalPerson>) -> Backpack {
        self.person_id2backpack
            .remove(person_id)
            .unwrap_or_else(|| {
                panic!("Tried to remove an agent, for which no backpack is available!")
            })
    }

    pub fn get_backpacks(&self) -> &HashMap<Id<InternalPerson>, Backpack> {
        &self.person_id2backpack
    }

    pub fn get_vehicles(&self) -> &HashMap<Id<InternalVehicle>, HashSet<Id<InternalPerson>>> {
        &self.vehicle_id2person_ids
    }

    /// This method's main purpose is to forward relevant events to the backpacks affected by given event.
    /// Events which do not affect the Backpack of any person will be ignored.
    /// TODO This method is quite clunky as there is no HasPersonId/HasVehicleId trait as there is in Java MATSim. Adding a trait could make the function much easier. Ask PH.
    fn handle_event(&mut self, event: &dyn EventTrait) {
        let affected_persons = if let Some(e) = event.as_any().downcast_ref::<LinkEnterEvent>() {
            match self.vehicle_id2person_ids.get(&e.vehicle) {
                Some(persons) => persons.iter().cloned().collect(),
                // The vehicle-to-person mapping arrives one step after the vehicle body (broker
                // AfterSimStep → BeforeSimStep). Buffer for replay once both the mapping and the
                // backpack are present (see replay_deferred_link_events).
                None if self.pending_vehicles.contains(&e.vehicle) => {
                    self.deferred_link_events.push(e.clone());
                    return;
                }
                None => return, // untracked vehicle (e.g. teleportation)
            }
        } else if let Some(e) = event.as_any().downcast_ref::<PersonArrivalEvent>() {
            vec![e.person.clone()]
        } else if let Some(e) = event.as_any().downcast_ref::<PersonDepartureEvent>() {
            vec![e.person.clone()]
        } else if let Some(e) = event.as_any().downcast_ref::<ActivityStartEvent>() {
            vec![e.person.clone()]
        } else if let Some(e) = event.as_any().downcast_ref::<ActivityEndEvent>() {
            vec![e.person.clone()]
        } else if let Some(e) = event.as_any().downcast_ref::<TeleportationArrivalEvent>() {
            vec![e.person.clone()]
        } else if let Some(e) = event.as_any().downcast_ref::<PersonEntersVehicleEvent>() {
            vec![e.person.clone()]
        } else if let Some(e) = event.as_any().downcast_ref::<PersonLeavesVehicleEvent>() {
            vec![e.person.clone()]
        } else if let Some(e) = event.as_any().downcast_ref::<VehicleEntersTrafficEvent>() {
            self.vehicle_id2person_ids
                .get(&e.vehicle)
                .map(|persons| persons.iter().cloned().collect())
                .unwrap_or_default()
        } else if let Some(e) = event.as_any().downcast_ref::<VehicleLeavesTrafficEvent>() {
            self.vehicle_id2person_ids
                .get(&e.vehicle)
                .map(|persons| persons.iter().cloned().collect())
                .unwrap_or_default()
        } else {
            return;
        };

        affected_persons.into_iter().for_each(|person| {
            self.person_id2backpack
                .get_mut(&person)
                .unwrap()
                .handle_event(event);
        });
    }

    pub(crate) fn register_event_fn(
        data_collector: Arc<Mutex<BackpackingDataCollector>>,
    ) -> Box<EventHandlerRegisterFn> {
        Box::new(move |events: &mut EventsManager| {
            // General backpacking event forwarding
            let data_collector1 = Arc::clone(&data_collector);
            events.on_any(move |e: &dyn EventTrait| {
                let mut bdc = data_collector1.lock().unwrap();
                bdc.handle_event(e);
            });

            // Events for Vehicle2Person mappings
            let data_collector2 = Arc::clone(&data_collector);
            events.on::<PersonEntersVehicleEvent, _>(move |e: &PersonEntersVehicleEvent| {
                let mut bdc = data_collector2.lock().unwrap();
                // println!("Partition #{}: Entered vehicle {}", bdc.rank, e.vehicle);
                bdc.vehicle_id2person_ids
                    .entry(e.vehicle.clone())
                    .or_default()
                    .insert(e.person.clone());
            });

            let data_collector3 = Arc::clone(&data_collector);
            events.on::<PersonLeavesVehicleEvent, _>(move |e: &PersonLeavesVehicleEvent| {
                let mut bdc = data_collector3.lock().unwrap();
                // println!("Partition #{}: Left vehicle {}", bdc.rank, e.vehicle);
                bdc.vehicle_id2person_ids.remove(&e.vehicle);
            });
        })
    }

    pub(crate) fn register_partition_fn(
        data_collector: Arc<Mutex<BackpackingDataCollector>>,
    ) -> Box<PartitionListenerRegisterFn> {
        Box::new(move |events: &mut PartitionEventsManager| {
            let data_collector1 = Arc::clone(&data_collector);
            events.on_event(move |e: &RuntimeEvent<PartitionEvent>| match &e.payload {
                PartitionEvent::VehicleLeavesPartition(i) => {
                    let mut bdc = data_collector1.lock().unwrap();

                    let leaving_vehicle = bdc.remove_leaving_vehicles(&i.vehicle_id);
                    bdc.message_broker.lock().unwrap().add_leaving_vehicle(
                        i.to.clone(),
                        i.vehicle_id.clone(),
                        leaving_vehicle,
                    );
                }
                PartitionEvent::AgentLeavesPartition(i) => {
                    let mut bdc = data_collector1.lock().unwrap();

                    let leaving_backpack = bdc.remove_leaving_backpack(&i.agent_id);
                    bdc.message_broker.lock().unwrap().add_leaving_backpack(
                        i.to.clone(),
                        i.agent_id.clone(),
                        leaving_backpack,
                    );
                }
                PartitionEvent::AgentEntersPartition(i) => {
                    data_collector1
                        .lock()
                        .unwrap()
                        .message_broker
                        .lock()
                        .unwrap()
                        .wait_for_backpack(i.agent_id.clone());
                }
                PartitionEvent::VehicleEntersPartition(i) => {
                    let mut bdc = data_collector1.lock().unwrap();
                    bdc.pending_vehicles.insert(i.vehicle_id.clone());
                    bdc.message_broker
                        .lock()
                        .unwrap()
                        .wait_for_vehicle(i.vehicle_id.clone());
                }
            });
        })
    }

    pub(crate) fn prepare_send_to_home(&mut self) {
        let mut leaving_person_ids: Vec<_> = Vec::default();

        // Send foreign backpacks to their home partition
        for (person, backpack) in self.person_id2backpack.iter() {
            if backpack.get_starting_partion() != self.rank {
                leaving_person_ids.push(person.clone());
            }
        }

        for person_id in leaving_person_ids.drain(..) {
            let leaving_backpack = self.remove_leaving_backpack(&person_id);
            self.message_broker.lock().unwrap().add_leaving_backpack(
                leaving_backpack.get_starting_partion(),
                person_id,
                leaving_backpack,
            );
        }
    }

    pub(crate) fn finish(&mut self) -> Population {
        let persons: HashMap<Id<InternalPerson>, InternalPerson> = self
            .person_id2backpack
            .drain()
            .map(|(person_id, backpack)| (person_id, backpack.finish()))
            .collect();

        Population { persons }
    }
}
