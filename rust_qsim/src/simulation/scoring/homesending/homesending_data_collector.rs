use crate::simulation::events::{
    ActivityEndEvent, ActivityStartEvent, EventHandlerRegisterFn, EventTrait, EventsManager,
    LinkEnterEvent, PersonArrivalEvent, PersonDepartureEvent, PersonEntersVehicleEvent,
    PersonLeavesVehicleEvent, TeleportationArrivalEvent, VehicleEntersTrafficEvent,
    VehicleLeavesTrafficEvent,
};
use crate::simulation::framework_events::{
    PartitionEvent, PartitionListenerRegisterFn, QSimId, RuntimeEvent,
};
use crate::simulation::id::Id;
use crate::simulation::scenario::population::{InternalPerson, Population};
use crate::simulation::scenario::vehicles::InternalVehicle;
use crate::simulation::scoring::homesending::homesending_message_broker::HomeSendingMessageBroker;
use crate::simulation::scoring::partial_plans::PartialPlan;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

pub struct HomeSendingDataCollector {
    person_id2home_partition: HashMap<Id<InternalPerson>, QSimId>,
    rank: QSimId,

    message_broker: Arc<Mutex<HomeSendingMessageBroker>>,

    person_id2events: HashMap<Id<InternalPerson>, Vec<Box<dyn EventTrait>>>,
    vehicle_id2person_ids: HashMap<Id<InternalVehicle>, HashSet<Id<InternalPerson>>>,
}

impl HomeSendingDataCollector {
    pub fn new(
        population: &Population,
        person_id2home_partition: HashMap<Id<InternalPerson>, QSimId>,
        rank: QSimId,
        message_broker: Arc<Mutex<HomeSendingMessageBroker>>,
    ) -> Arc<Mutex<Self>> {
        let data_collector = Arc::new(Mutex::new(Self {
            person_id2home_partition,
            rank,
            message_broker,
            person_id2events: HashMap::new(),
            vehicle_id2person_ids: HashMap::new(),
        }));
        data_collector
            .lock()
            .unwrap()
            .generate_event_vectors_for_population(&population);
        data_collector
    }

    fn generate_event_vectors_for_population(&mut self, population: &Population) {
        for person in population.persons.keys() {
            self.person_id2events.insert(person.clone(), Vec::default());
        }
    }

    pub(crate) fn is_person_at_home(&self, person_id: &Id<InternalPerson>) -> bool {
        *self.person_id2home_partition.get(person_id).unwrap() == self.rank
    }

    pub(crate) fn add_arriving_vehicles(
        &mut self,
        arriving_vehicles: HashMap<Id<InternalVehicle>, HashSet<Id<InternalPerson>>>,
    ) {
        self.vehicle_id2person_ids.extend(arriving_vehicles);
    }

    /// Adds the events to the corresponding event vectors. Assumes, that events as well as the
    /// calls of this function are already sorted! Delivering unsorted events or blocks will cause
    /// the scoring module to panic!
    pub(crate) fn add_arriving_events(
        &mut self,
        person_id: Id<InternalPerson>,
        arriving_events: Vec<Box<dyn EventTrait>>,
    ) {
        self.person_id2events
            .get_mut(&person_id)
            .unwrap()
            .extend(arriving_events);
    }

    pub(crate) fn remove_leaving_vehicles(
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

    /// This method's main purpose is to forward relevant events to the plan affected by given event.
    /// Events which do not affect the Plan of any person will be ignored.
    /// TODO This method is quite clunky as there is no HasPersonId/HasVehicleId trait as there is in Java MATSim. Adding a trait could make the function much easier. Ask PH.
    fn handle_event(&mut self, event: &dyn EventTrait) {
        let affected_persons: Vec<(Id<InternalPerson>, Box<dyn EventTrait>)> =
            if let Some(e) = event.as_any().downcast_ref::<LinkEnterEvent>() {
                self.vehicle_id2person_ids
                    .get(&e.vehicle)
                    .into_iter()
                    .flatten()
                    .cloned()
                    .map(|person| (person, Box::new(e.clone()) as Box<dyn EventTrait>))
                    .collect::<Vec<_>>()
            } else if let Some(e) = event.as_any().downcast_ref::<PersonArrivalEvent>() {
                vec![(e.person.clone(), Box::new(e.clone()))]
            } else if let Some(e) = event.as_any().downcast_ref::<PersonDepartureEvent>() {
                vec![(e.person.clone(), Box::new(e.clone()))]
            } else if let Some(e) = event.as_any().downcast_ref::<ActivityStartEvent>() {
                vec![(e.person.clone(), Box::new(e.clone()))]
            } else if let Some(e) = event.as_any().downcast_ref::<ActivityEndEvent>() {
                vec![(e.person.clone(), Box::new(e.clone()))]
            } else if let Some(e) = event.as_any().downcast_ref::<TeleportationArrivalEvent>() {
                vec![(e.person.clone(), Box::new(e.clone()))]
            } else if let Some(e) = event.as_any().downcast_ref::<PersonEntersVehicleEvent>() {
                vec![(e.person.clone(), Box::new(e.clone()))]
            } else if let Some(e) = event.as_any().downcast_ref::<PersonLeavesVehicleEvent>() {
                vec![(e.person.clone(), Box::new(e.clone()))]
            } else if let Some(e) = event.as_any().downcast_ref::<VehicleEntersTrafficEvent>() {
                self.vehicle_id2person_ids
                    .get(&e.vehicle)
                    .into_iter()
                    .flatten()
                    .cloned()
                    .map(|person| (person, Box::new(e.clone()) as Box<dyn EventTrait>))
                    .collect::<Vec<_>>()
            } else if let Some(e) = event.as_any().downcast_ref::<VehicleLeavesTrafficEvent>() {
                self.vehicle_id2person_ids
                    .get(&e.vehicle)
                    .into_iter()
                    .flatten()
                    .cloned()
                    .map(|person| (person, Box::new(e.clone()) as Box<dyn EventTrait>))
                    .collect::<Vec<_>>()
            } else {
                return;
            };

        affected_persons
            .into_iter()
            .for_each(move |(person_id, boxed_event)| {
                let target = self.person_id2home_partition.get(&person_id).unwrap();

                if *target == self.rank {
                    // For full correctness, the events need to pass the arriving_events buffer in the
                    // message broker. Use the internal method to bypass the message sending
                    self.message_broker.lock().unwrap().push_events_on_block(
                        person_id,
                        self.rank,
                        vec![boxed_event],
                    );
                } else {
                    self.message_broker.lock().unwrap().add_leaving_event(
                        *target,
                        person_id,
                        boxed_event,
                    );
                }
            });
    }

    pub(crate) fn register_event_fn(
        data_collector: Arc<Mutex<HomeSendingDataCollector>>,
    ) -> Box<EventHandlerRegisterFn> {
        Box::new(move |events: &mut EventsManager| {
            // General event forwarding
            let collector1 = Arc::clone(&data_collector);
            events.on_any(move |e: &dyn EventTrait| {
                let mut hdc = collector1.lock().unwrap();
                hdc.handle_event(e);
            });

            // Events for Vehicle2Person mappings
            let collector2 = Arc::clone(&data_collector);
            events.on::<PersonEntersVehicleEvent, _>(move |e: &PersonEntersVehicleEvent| {
                let mut bdc = collector2.lock().unwrap();
                bdc.vehicle_id2person_ids
                    .entry(e.vehicle.clone())
                    .or_default()
                    .insert(e.person.clone());
            });

            let collector3 = Arc::clone(&data_collector);
            events.on::<PersonLeavesVehicleEvent, _>(move |e: &PersonLeavesVehicleEvent| {
                let mut bdc = collector3.lock().unwrap();
                bdc.vehicle_id2person_ids.remove(&e.vehicle);
            });
        })
    }

    pub(crate) fn register_partition_fn(
        data_collector: Arc<Mutex<HomeSendingDataCollector>>,
    ) -> Box<PartitionListenerRegisterFn> {
        Box::new(move |events| {
            let data_collector1 = Arc::clone(&data_collector);
            events.on_event(move |e: &RuntimeEvent<PartitionEvent>| match &e.payload {
                PartitionEvent::VehicleLeavesPartition(i) => {
                    let mut hdc = data_collector1.lock().unwrap();

                    let leaving_vehicle = hdc.remove_leaving_vehicles(&i.vehicle_id);
                    hdc.message_broker.lock().unwrap().add_leaving_vehicle(
                        i.to.clone(),
                        i.vehicle_id.clone(),
                        leaving_vehicle,
                    );
                }
                PartitionEvent::AgentLeavesPartition(i) => {
                    let hdc = data_collector1.lock().unwrap();

                    // TODO Calling close_block causes a deadlock, therefore the current fix is
                    //      to let the message broker send a message to itself. Try to find a
                    //      cleaner solution.
                    /*
                    if hdc.is_person_at_home(&i.agent_id) {
                        // If this agent is currently in its home partition, there is no need to
                        // send a leave message, as the events are already processed locally.
                        hdc.message_broker.lock().unwrap().close_block(
                            i.agent_id.clone(),
                            hdc.rank,
                            Some(i.clone()),
                        );
                        return;
                    }
                    */

                    let home_partition = hdc.person_id2home_partition.get(&i.agent_id).unwrap();
                    hdc.message_broker
                        .lock()
                        .unwrap()
                        .add_leaving_partition_event(
                            *home_partition,
                            i.agent_id.clone(),
                            e.payload.clone(),
                        )
                }
                PartitionEvent::AgentEntersPartition(i) => {
                    let hdc = data_collector1.lock().unwrap();

                    if hdc.is_person_at_home(&i.agent_id) {
                        // If this agent is currently in its home partition, there is no need to
                        // send a leave message, as the events are already processed locally.
                        hdc.message_broker.lock().unwrap().open_block(
                            i.agent_id.clone(),
                            hdc.rank,
                            Some(i.clone()),
                        );
                        return;
                    }

                    let home_partition = hdc.person_id2home_partition.get(&i.agent_id).unwrap();
                    hdc.message_broker
                        .lock()
                        .unwrap()
                        .add_leaving_partition_event(
                            *home_partition,
                            i.agent_id.clone(),
                            e.payload.clone(),
                        )
                }
                _ => {}
            });
        })
    }

    pub(crate) fn finish(&mut self) -> Population {
        let persons: HashMap<Id<InternalPerson>, InternalPerson> = self
            .person_id2events
            .drain()
            .map(|(person_id, events)| {
                let mut plan = PartialPlan::default();

                for event in events {
                    plan.handle_event(&*event);
                }

                (
                    person_id.clone(),
                    InternalPerson::new(person_id, plan.finish()),
                )
            })
            .collect();

        Population { persons }
    }
}
