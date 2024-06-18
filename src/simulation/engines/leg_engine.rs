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
use crate::simulation::wire_types::population::attribute_value::Type;
use crate::simulation::wire_types::population::{Leg, Person};
use crate::simulation::wire_types::vehicles::LevelOfDetail;
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
    waiting_passengers: IntMap<u64, Person>,
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
            waiting_passengers: IntMap::default(),
        }
    }

    pub(crate) fn do_step(&mut self, now: u32) {
        let teleported_vehicles = self.teleportation_engine.do_step(now);
        let network_vehicles = self.network_engine.move_nodes(now);

        let mut agents = vec![];

        for veh in network_vehicles.into_iter().chain(teleported_vehicles) {
            self.events
                .borrow_mut()
                .publish_event(now, &Event::new_person_leaves_veh(veh.driver().id, veh.id));

            for passenger in veh.passengers() {
                self.events.borrow_mut().publish_event(
                    now,
                    &Event::new_passenger_dropped_off(
                        passenger.id,
                        passenger.curr_leg().mode,
                        0, //TODO
                        veh.id,
                    ),
                );
                self.events
                    .borrow_mut()
                    .publish_event(now, &Event::new_person_leaves_veh(passenger.id, veh.id));
            }

            agents.extend(self.garage.park_veh(veh));
        }

        for mut agent in agents {
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

        if let Some(vehicle) = vehicle {
            self.pass_vehicle_to_engine(now, vehicle, true);
        }
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
        waiting_passengers: &mut IntMap<u64, Person>,
    ) -> Option<Vehicle>;
}

struct VehicularDepartureHandler {
    events: Rc<RefCell<EventsPublisher>>,
}

impl VehicularDepartureHandler {
    pub fn new(events: Rc<RefCell<EventsPublisher>>) -> Self {
        VehicularDepartureHandler { events }
    }
}

impl DepartureHandler for VehicularDepartureHandler {
    fn handle_departure(
        &mut self,
        now: u32,
        agent: Person,
        garage: &mut Garage,
        _: &mut IntMap<u64, Person>,
    ) -> Option<Vehicle> {
        assert_ne!(agent.curr_plan_elem % 2, 0);

        let leg = agent.curr_leg();
        let route = leg.route.as_ref().unwrap();
        let leg_mode: Id<String> = Id::get(leg.mode);
        let veh_id = Id::get(route.veh_id);

        self.events.borrow_mut().publish_event(
            now,
            &Event::new_departure(agent.id, route.start_link(), leg_mode.internal()),
        );
        self.events.borrow_mut().publish_event(
            now,
            &Event::new_person_enters_veh(agent.id, veh_id.internal()),
        );

        Some(garage.unpark_veh(agent, &veh_id))
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
        _: &mut Garage,
        waiting_passengers: &mut IntMap<u64, Person>,
    ) -> Option<Vehicle> {
        let act_before = agent.previous_act();
        let leg = agent.curr_leg();
        let leg_mode: Id<String> = Id::get(leg.mode);
        self.events.borrow_mut().publish_event(
            now,
            &Event::new_departure(agent.id, act_before.link_id, leg_mode.internal()),
        );

        waiting_passengers.insert(agent.id, agent);
        None
    }
}

struct DrtDriverDepartureHandler {
    events: Rc<RefCell<EventsPublisher>>,
}

impl DepartureHandler for DrtDriverDepartureHandler {
    fn handle_departure(
        &mut self,
        now: u32,
        agent: Person,
        garage: &mut Garage,
        waiting_passengers: &mut IntMap<u64, Person>,
    ) -> Option<Vehicle> {
        // remove passenger from waiting queue, place driver and passenger in vehicle and hand it over to leg engine
        let passenger_id = match agent
            .curr_leg()
            .attributes
            .get(Leg::PASSENGER_ID_ATTRIBUTE)
            .expect("No passenger id found")
            .r#type
            .as_ref()
            .unwrap()
        {
            Type::IntValue(id) => id,
            Type::StringValue(_) => {
                unreachable!()
            }
            Type::DoubleValue(_) => {
                unreachable!()
            }
        };

        let passengers: Vec<Person> = vec![waiting_passengers
            .remove(&passenger_id)
            .expect("No such passenger is waiting.")];

        let leg = agent.curr_leg();
        let route = leg.route.as_ref().unwrap();
        let leg_mode: Id<String> = Id::get(leg.mode);
        let veh_id = agent.curr_leg().route.as_ref().unwrap().veh_id;

        // emit events for passengers
        for passenger in &passengers {
            let mode = passenger.curr_leg().mode;
            self.events
                .borrow_mut()
                .publish_event(now, &Event::new_person_enters_veh(passenger.id, veh_id));
            self.events.borrow_mut().publish_event(
                now,
                &Event::new_passenger_picked_up(passenger.id, mode, 0, veh_id),
            );
        }

        // emit event for driver
        self.events.borrow_mut().publish_event(
            now,
            &Event::new_departure(agent.id, route.start_link(), leg_mode.internal()),
        );

        Some(garage.unpark_veh_with_passengers(agent, passengers, &Id::get(veh_id)))
    }
}

trait ArrivalHandler {
    fn handle_arrival(&mut self, now: u32, agent: Person, garage: &mut Garage);
}
