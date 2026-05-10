use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use crate::simulation::events::{ActivityEndEvent, ActivityStartEvent, EventHandlerRegisterFn, EventTrait, EventsManager, LinkEnterEvent, LinkLeaveEvent, PersonEntersVehicleEvent, PersonLeavesVehicleEvent};
use crate::simulation::framework_events::{AgentLeavesPartitionEvent, PartitionEvent, PartitionListenerRegisterFn, RuntimeEvent};
use crate::simulation::id::Id;
use crate::simulation::scenario::network::Link;
use crate::simulation::scenario::population::{InternalPerson, Population};
use crate::simulation::scenario::vehicles::InternalVehicle;
use crate::simulation::scoring::backpacking::backpack::Backpack;
use crate::simulation::scoring::backpacking::backpacking_scoring_broker::BackpackingMessageBroker;

pub struct BackpackingDataCollector {
    person_id2backpack: HashMap<Id<InternalPerson>, Backpack>,
    vehicle_id2person_ids: HashMap<Id<InternalVehicle>, HashSet<Id<InternalPerson>>>,
    cut_link_id2target_partition: HashMap<Id<Link>, u32>,
    rank: u32,

    message_broker: Arc<Mutex<BackpackingMessageBroker>>,
}

impl BackpackingDataCollector {
    pub fn new(
        population: &Population,
        cut_link_id2target_partition: HashMap<Id<Link>, u32>,
        rank: u32,
        message_broker: Arc<Mutex<BackpackingMessageBroker>>
    ) -> Arc<Mutex<Self>>
    {
        let data_collector = Arc::new(Mutex::new(Self {
            person_id2backpack: Default::default(),
            vehicle_id2person_ids: Default::default(),
            cut_link_id2target_partition,
            rank,
            message_broker
        }));
        data_collector.lock().unwrap().generate_backpacks_for_population(&population);
        data_collector
    }

    fn generate_backpacks_for_population(&mut self, population: &Population){
        for person in population.persons.iter(){
            self.person_id2backpack.insert(person.0.clone(), Backpack::new(person.0.clone(), self.rank));
        }
    }

    fn add_special_scoring_event(&mut self, person: &Id<InternalPerson>, event: Box<dyn EventTrait>) {
        println!("Partition #{}: Adding special scoring event for id {}", self.rank, person); // TODO Debug only, remove when working

        self.person_id2backpack
            .get_mut(person)
            .unwrap()
            .add_special_scoring_event(event);
    }

    pub(crate) fn add_arriving_backpacks(&mut self, arriving_passengers: HashMap<Id<InternalPerson>, Backpack>) {
        for k in arriving_passengers.keys(){
            println!("Partition #{}: Adding arriving passenger {}", self.rank, k); // TODO Debug only, remove when working
        }
        self.person_id2backpack.extend(arriving_passengers);
    }

    fn remove_leaving_backpack(&mut self, person_id: Id<InternalPerson>) -> Backpack {
        self.person_id2backpack.remove(&person_id).unwrap_or_else(|| {panic!("Tried to remove an agent, for which no backpack is available")})
    }

    pub fn get_backpacks(&self) -> &HashMap<Id<InternalPerson>, Backpack> {
        &self.person_id2backpack
    }

    pub(crate) fn register_event_fn(data_collector: Arc<Mutex<BackpackingDataCollector>>) -> Box<EventHandlerRegisterFn> {
        Box::new(move |events: &mut EventsManager| {
            let bdc1 = Arc::clone(&data_collector);
            events.on::<ActivityStartEvent, _>(move |e: &ActivityStartEvent| {
                let mut bdc = bdc1.lock().unwrap();
                println!("Partition #{}: Person {} starts activity {}", bdc.rank, e.person.clone(), e.act_type.clone());
                bdc.add_special_scoring_event(&e.person, Box::new(e.clone()));
            });

            let bdc2 = Arc::clone(&data_collector);
            events.on::<ActivityEndEvent, _>(move |e: &ActivityEndEvent| {
                let mut bdc = bdc2.lock().unwrap();
                println!("Partition #{}: Person {} ends activity {}", bdc.rank, e.person.clone(), e.act_type.clone());
                bdc.add_special_scoring_event(&e.person, Box::new(e.clone()));
            });
        })
    }

    pub(crate) fn register_partition_fn(data_collector: Arc<Mutex<BackpackingDataCollector>>) -> Box<PartitionListenerRegisterFn> {
        Box::new(move |events| {
            let bdc = Arc::clone(&data_collector);
            events.on_event(move |e: &RuntimeEvent<PartitionEvent>| {
                match &e.payload {
                    PartitionEvent::AgentLeavesPartition(i) => {
                        let leaving_backpack = bdc.lock().unwrap().remove_leaving_backpack(i.agent_id.clone());
                        bdc.lock().unwrap().message_broker.lock().unwrap().add_leaving_backpack(i.to.clone(), i.agent_id.clone(), leaving_backpack);
                    },
                    _ => {}
                }
            });
        })
    }
}