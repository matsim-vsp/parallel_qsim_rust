use crate::simulation::events::{ActivityEndEvent, ActivityStartEvent, EventHandlerRegisterFn, EventTrait, EventsManager, LinkEnterEvent, PersonArrivalEvent, PersonDepartureEvent, PersonEntersVehicleEvent, PersonLeavesVehicleEvent, TeleportationArrivalEvent, VehicleEntersTrafficEvent, VehicleLeavesTrafficEvent};
use crate::simulation::framework_events::{PartitionEvent, PartitionListenerRegisterFn, QSimId, RuntimeEvent};
use crate::simulation::id::Id;
use crate::simulation::scenario::population::{InternalPerson, Population};
use crate::simulation::scenario::vehicles::InternalVehicle;
use crate::simulation::scoring::homesending::homesending_message_broker::HomeSendingMessageBroker;
use crate::simulation::scoring::partial_plans::PartialPlan;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

pub struct HomeSendingDataCollector {
    person_id2partial_plan: HashMap<Id<InternalPerson>, PartialPlan>,
    person_id2home_partition: HashMap<Id<InternalPerson>, QSimId>,
    vehicle_id2person_ids: HashMap<Id<InternalVehicle>, HashSet<Id<InternalPerson>>>,
    rank: QSimId,

    message_broker: Arc<Mutex<HomeSendingMessageBroker>>,
}

impl HomeSendingDataCollector {
    pub fn new(
        population: &Population,
        person_id2home_partition: HashMap<Id<InternalPerson>, QSimId>,
        rank: QSimId,
        message_broker: Arc<Mutex<HomeSendingMessageBroker>>,
    ) -> Arc<Mutex<Self>>
    {
        let data_collector = Arc::new(Mutex::new(Self {
            person_id2partial_plan: Default::default(),
            person_id2home_partition,
            vehicle_id2person_ids: Default::default(),
            rank,
            message_broker
        }));
        data_collector.lock().unwrap().generate_partial_plans_for_population(&population);
        data_collector
    }

    fn generate_partial_plans_for_population(&mut self, population: &Population) {
        for person in population.persons.keys() {
            self.person_id2partial_plan.insert(person.clone(), PartialPlan::default());
        }
    }

    pub(crate) fn add_arriving_vehicles(&mut self, arriving_vehicles: HashMap<Id<InternalVehicle>, HashSet<Id<InternalPerson>>>) {
        self.vehicle_id2person_ids.extend(arriving_vehicles);
    }

    pub(crate) fn add_arriving_events(&mut self, arriving_events: HashMap<Id<InternalPerson>, Box<dyn EventTrait>>) {
        println!("Partition #{}: Handling arriving message", self.rank);
        arriving_events.into_iter().for_each(|(person_id, arriving_event)| {
            self.person_id2partial_plan.get_mut(&person_id).unwrap().handle_event(&*arriving_event);
        })
    }

    fn remove_leaving_vehicles(&mut self, vehicle_id: &Id<InternalVehicle>) -> HashSet<Id<InternalPerson>> {
        // TODO Build a checker, so that it only allows missing entries for teleported modes
        self.vehicle_id2person_ids.remove(vehicle_id).unwrap_or_else(|| {
            // warn!("Partition #{}: Tried to remove vehicle {}, which has no entry!", self.rank, vehicle_id);
            return HashSet::default()
        })
    }

    /// This method's main purpose is to forward relevant events to the plan affected by given event.
    /// Events which do not affect the Plan of any person will be ignored.
    /// TODO This method is quite clunky as there is no HasPersonId/HasVehicleId trait as there is in Java MATSim. Adding a trait could make the function much easier. Ask PH.
    fn handle_event(&mut self, event: &dyn EventTrait ) {
        let affected_persons: Vec<(Id<InternalPerson>, Box<dyn EventTrait>)> = if let Some(e) = event.as_any().downcast_ref::<LinkEnterEvent>() {
            println!("LinkEnterEvent");
            self.vehicle_id2person_ids
                .get(&e.vehicle)
                .into_iter()
                .flatten()
                .cloned()
                .map(|person| (person, Box::new(e.clone()) as Box<dyn EventTrait>))
                .collect::<Vec<_>>()
        } else if let Some(e) = event.as_any().downcast_ref::<PersonArrivalEvent>() {
            println!("PersonArrivalEvent");
            vec![(e.person.clone(), Box::new(e.clone()))]
        } else if let Some(e) = event.as_any().downcast_ref::<PersonDepartureEvent>() {
            println!("PersonDepartureEvent");
            vec![(e.person.clone(), Box::new(e.clone()))]
        } else if let Some(e) = event.as_any().downcast_ref::<ActivityStartEvent>() {
            println!("ActivityStartEvent");
            vec![(e.person.clone(), Box::new(e.clone()))]
        } else if let Some(e) = event.as_any().downcast_ref::<ActivityEndEvent>() {
            println!("ActivityEndEvent");
            vec![(e.person.clone(), Box::new(e.clone()))]
        } else if let Some(e) = event.as_any().downcast_ref::<TeleportationArrivalEvent>() {
            println!("TeleportationArrivalEvent");
            vec![(e.person.clone(), Box::new(e.clone()))]
        } else if let Some(e) = event.as_any().downcast_ref::<PersonEntersVehicleEvent>() {
            println!("PersonEntersVehicleEvent");
            vec![(e.person.clone(), Box::new(e.clone()))]
        } else if let Some(e) = event.as_any().downcast_ref::<PersonLeavesVehicleEvent>() {
            println!("PersonLeavesVehicleEvent");
            vec![(e.person.clone(), Box::new(e.clone()))]
        } else if let Some(e) = event.as_any().downcast_ref::<VehicleEntersTrafficEvent>() {
            println!("VehicleEntersTrafficEvent");
            self.vehicle_id2person_ids
                .get(&e.vehicle)
                .into_iter()
                .flatten()
                .cloned()
                .map(|person| (person, Box::new(e.clone()) as Box<dyn EventTrait>))
                .collect::<Vec<_>>()
        } else if let Some(e) = event.as_any().downcast_ref::<VehicleLeavesTrafficEvent>() {
            println!("VehicleLeavesTrafficEvent");
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

        affected_persons.into_iter().for_each(move |(person, boxed_event)| {
            let target = self.person_id2home_partition.get(&person).unwrap();

            if *target == self.rank {
                println!("Partition #{}: Handling event", self.rank);
                self.person_id2partial_plan.get_mut(&person).unwrap().handle_event(event);
            } else {
                println!("Partition #{}: Sending event to #{}", self.rank, *target);
                self.message_broker.lock().unwrap().add_leaving_event(*target, person, boxed_event);
            }
        });
    }

    pub(crate) fn register_event_fn(data_collector: Arc<Mutex<HomeSendingDataCollector>>) -> Box<EventHandlerRegisterFn> {
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

    pub(crate) fn register_partition_fn(data_collector: Arc<Mutex<HomeSendingDataCollector>>) -> Box<PartitionListenerRegisterFn> {
        Box::new(move |events| {
            let data_collector1 = Arc::clone(&data_collector);
            events.on_event(move |e: &RuntimeEvent<PartitionEvent>| {
                match &e.payload {
                    PartitionEvent::VehicleLeavesPartition(i) => {
                        let leaving_vehicle = data_collector1.lock().unwrap().remove_leaving_vehicles(&i.vehicle_id);
                        data_collector1.lock().unwrap().message_broker.lock().unwrap().add_leaving_vehicle(i.to.clone(), i.vehicle_id.clone(), leaving_vehicle);
                    }
                    _ => {}
                }
            });
        })
    }

    pub(crate) fn finish(&mut self) -> Population {
        let persons: HashMap<Id<InternalPerson>, InternalPerson> = self.person_id2partial_plan.drain().map(|(person_id, partial_plan)| {
            (person_id.clone(), InternalPerson::new(person_id, partial_plan.finish()))
        }).collect();

        Population {
            persons
        }
    }
}