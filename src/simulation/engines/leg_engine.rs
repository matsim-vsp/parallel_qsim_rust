use crate::simulation::engines::network_engine::NetworkEngine;
use crate::simulation::engines::teleportation_engine::TeleportationEngine;
use crate::simulation::engines::AgentStateTransitionLogic;
use crate::simulation::id::Id;
use crate::simulation::messaging::communication::communicators::SimCommunicator;
use crate::simulation::messaging::communication::message_broker::NetMessageBroker;
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::network::sim_network::SimNetworkPartition;
use crate::simulation::vehicles::garage::Garage;
use crate::simulation::wire_types::events::Event;
use crate::simulation::wire_types::messages::Vehicle;
use crate::simulation::wire_types::population::Person;
use crate::simulation::wire_types::vehicles::LevelOfDetail;
use ahash::HashSet;
use nohash_hasher::IntMap;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

pub struct LegEngine<C: SimCommunicator> {
    teleportation_engine: TeleportationEngine,
    network_engine: NetworkEngine,
    garage: Garage,
    net_message_broker: NetMessageBroker<C>,
    events: Rc<RefCell<EventsPublisher>>,
    agent_state_transition_logic: Weak<RefCell<AgentStateTransitionLogic<C>>>,
    departure_handler: IntMap<u64, Box<dyn DepartureHandler>>,
    waiting_passengers: HashSet<u64>,
}

impl<C: SimCommunicator + 'static> LegEngine<C> {
    pub fn new(
        network: SimNetworkPartition,
        garage: Garage,
        net_message_broker: NetMessageBroker<C>,
        events: Rc<RefCell<EventsPublisher>>,
    ) -> Self {
        LegEngine {
            teleportation_engine: TeleportationEngine::new(events.clone()),
            network_engine: NetworkEngine::new(network, events.clone()),
            agent_state_transition_logic: Weak::new(),
            garage,
            net_message_broker,
            events,
            departure_handler: IntMap::default(), //TODO
            waiting_passengers: HashSet::default(),
        }
    }

    pub(crate) fn do_step(&mut self, now: u32) {
        let teleported_agents = self.teleportation_engine.do_step(now);
        let network_agents = self.network_engine.move_nodes(now, &mut self.garage);

        for mut agent in teleported_agents.into_iter().chain(network_agents) {
            agent.advance_plan();

            self.agent_state_transition_logic
                .upgrade()
                .unwrap()
                .borrow_mut()
                .arrange_next_agent_state(now, agent);
        }

        self.network_engine
            .move_links(now, &mut self.net_message_broker);
        let sync_messages = self.net_message_broker.send_recv(now);

        for msg in sync_messages {
            self.network_engine
                .network
                .apply_storage_cap_updates(msg.storage_capacities);

            for veh in msg.vehicles {
                self.pass_vehicle_to_engine(now, veh, false);
            }
        }
    }

    pub(crate) fn receive_agent(&mut self, now: u32, agent: Person) {
        let d = self
            .departure_handler
            .get_mut(&agent.curr_leg().mode)
            .expect(&format!(
                "No departure handler for mode {:?}",
                &agent.curr_leg().mode
            ));
        let vehicle =
            d.handle_departure(now, agent, &mut self.garage, &mut self.waiting_passengers);
        self.pass_vehicle_to_engine(now, vehicle, true);
    }

    pub(crate) fn set_agent_state_transition_logic(
        &mut self,
        agent_state_transition_logic: Weak<RefCell<AgentStateTransitionLogic<C>>>,
    ) {
        self.agent_state_transition_logic = agent_state_transition_logic
    }

    pub fn agents(&mut self) -> Vec<&mut Person> {
        let mut agents = self.network_engine.network.active_agents();
        agents.extend(self.teleportation_engine.agents());
        agents
    }

    //TODO route begin is a bit hacky
    fn pass_vehicle_to_engine(&mut self, now: u32, vehicle: Vehicle, route_begin: bool) {
        let veh_type_id = Id::get(vehicle.r#type);
        let veh_type = self.garage.vehicle_types.get(&veh_type_id).unwrap();

        match veh_type.lod() {
            LevelOfDetail::Network => {
                self.network_engine
                    .receive_vehicle(now, vehicle, route_begin);
            }
            LevelOfDetail::Teleported => {
                self.teleportation_engine.receive_vehicle(
                    now,
                    vehicle,
                    &mut self.net_message_broker,
                );
            }
        }
    }

    pub fn net_message_broker(&self) -> &NetMessageBroker<C> {
        &self.net_message_broker
    }

    pub fn network(&self) -> &SimNetworkPartition {
        &self.network_engine.network
    }
}

trait DepartureHandler {
    fn handle_departure(
        &mut self,
        now: u32,
        agent: Person,
        garage: &mut Garage,
        waiting_passengers: &mut HashSet<u64>,
    ) -> Vehicle;
}

struct VehicularDepartureHandler<C: SimCommunicator> {
    events: Rc<RefCell<EventsPublisher>>,
    leg_engine: Weak<RefCell<LegEngine<C>>>,
}

impl<C: SimCommunicator + 'static> VehicularDepartureHandler<C> {
    pub fn new(events: Rc<RefCell<EventsPublisher>>) -> Self {
        VehicularDepartureHandler {
            events,
            leg_engine: Weak::new(), //TODO
        }
    }
}

impl<C: SimCommunicator + 'static> DepartureHandler for VehicularDepartureHandler<C> {
    fn handle_departure(
        &mut self,
        now: u32,
        agent: Person,
        garage: &mut Garage,
        waiting_passengers: &mut HashSet<u64>,
    ) -> Vehicle {
        assert_ne!(agent.curr_plan_elem % 2, 0);

        let leg = agent.curr_leg();
        let route = leg.route.as_ref().unwrap();
        let leg_mode: Id<String> = Id::get(leg.mode);
        self.events.borrow_mut().publish_event(
            now,
            &Event::new_departure(agent.id, route.start_link(), leg_mode.internal()),
        );

        let veh_id = Id::get(route.veh_id);
        garage.unpark_veh(agent, &veh_id)
    }
}

struct PassengerDepartureHandler {
    events: Rc<RefCell<EventsPublisher>>,
}

impl DepartureHandler for PassengerDepartureHandler {
    fn handle_departure(
        &mut self,
        now: u32,
        agent: Person,
        garage: &mut Garage,
        waiting_passengers: &mut HashSet<u64>,
    ) -> Vehicle {
        todo!()
        //place agent in dummy vehicle and hand it over to stop engine
    }
}

struct DrtDriverDepartureHandler<C: SimCommunicator> {
    events: Rc<RefCell<EventsPublisher>>,
    leg_engine: Weak<RefCell<LegEngine<C>>>,
}

impl<C: SimCommunicator + 'static> DepartureHandler for DrtDriverDepartureHandler<C> {
    fn handle_departure(
        &mut self,
        now: u32,
        agent: Person,
        garage: &mut Garage,
        waiting_passengers: &mut HashSet<u64>,
    ) -> Vehicle {
        // remove passenger from stop engine, place driver and passenger in vehicle and hand it over to leg engine
        // requirements:
        // 1. DrtDepartureHandler needs to know the passenger to be picked up
        let passenger: Vec<Person> = todo!();

        // 2. DrtDepartureHandler needs to be able to access the stop engine
        let stop_engine = todo!(); //not possible without handing it over (even with that I'm not sure)

        // 3. DrtDepartureHandler needs hand over the vehicle to the leg engine (right now done by the code calling the departure handler)
        let veh_id = agent.curr_leg().route.as_ref().unwrap().veh_id;
        garage.unpark_veh_with_passengers(agent, passenger, &Id::get(veh_id))
    }
}
