use crate::simulation::engines::network_engine::NetworkEngine;
use crate::simulation::engines::teleportation_engine::TeleportationEngine;
use crate::simulation::id::Id;
use crate::simulation::messaging::communication::communicators::SimCommunicator;
use crate::simulation::messaging::communication::message_broker::NetMessageBroker;
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::messaging::messages::SimulationAgentState;
use crate::simulation::network::sim_network::SimNetworkPartition;
use crate::simulation::vehicles::garage::Garage;
use crate::simulation::wire_types::events::Event;
use crate::simulation::wire_types::messages::{SimulationAgent, Vehicle};
use crate::simulation::wire_types::population::Person;
use crate::simulation::wire_types::vehicles::LevelOfDetail;
use std::cell::RefCell;
use std::rc::Rc;

pub struct LegEngine<C: SimCommunicator> {
    teleportation_engine: TeleportationEngine,
    network_engine: NetworkEngine,
    garage: Garage,
    net_message_broker: NetMessageBroker<C>,
    events: Rc<RefCell<EventsPublisher>>,
    departure_handler: VehicularDepartureHandler,
}

impl<C: SimCommunicator> LegEngine<C> {
    pub fn new(
        network: SimNetworkPartition,
        garage: Garage,
        net_message_broker: NetMessageBroker<C>,
        events: Rc<RefCell<EventsPublisher>>,
    ) -> Self {
        let departure_handler = VehicularDepartureHandler {
            events: events.clone(),
        };

        LegEngine {
            teleportation_engine: TeleportationEngine::new(events.clone()),
            network_engine: NetworkEngine::new(network, events.clone()),
            garage,
            net_message_broker,
            events,
            departure_handler,
        }
    }

    pub(crate) fn do_step(
        &mut self,
        now: u32,
        agents: Vec<SimulationAgent>,
    ) -> Vec<SimulationAgent> {
        for agent in agents {
            self.receive_agent(now, agent);
        }

        let teleported_vehicles = self.teleportation_engine.do_step(now);
        let network_vehicles = self.network_engine.move_nodes(now);

        let mut agents = vec![];
        agents.extend(self.publish_end_events(now, network_vehicles, true));
        agents.extend(self.publish_end_events(now, teleported_vehicles, false));

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

        agents
    }

    fn publish_end_events(
        &mut self,
        now: u32,
        vehicles: Vec<Vehicle>,
        publish_leave_vehicle: bool,
    ) -> Vec<SimulationAgent> {
        let mut agents = vec![];
        for veh in vehicles {
            //in case of teleportation, do not publish leave vehicle events
            if publish_leave_vehicle {
                self.events.borrow_mut().publish_event(
                    now,
                    &Event::new_person_leaves_veh(veh.driver().id(), veh.id),
                );
            }

            for passenger in veh.passengers() {
                self.events.borrow_mut().publish_event(
                    now,
                    &Event::new_passenger_dropped_off(
                        passenger.id(),
                        passenger.curr_leg().mode,
                        0, //TODO
                        veh.id,
                    ),
                );
                if publish_leave_vehicle {
                    self.events
                        .borrow_mut()
                        .publish_event(now, &Event::new_person_leaves_veh(passenger.id(), veh.id));
                }
            }

            let leg = veh.driver().curr_leg();
            let mode: Id<String> = Id::get(leg.mode);
            self.events.borrow_mut().publish_event(
                now,
                &Event::new_arrival(
                    veh.driver().id(),
                    veh.curr_link_id().unwrap(),
                    mode.internal(),
                ),
            );

            agents.extend(self.garage.park_veh(veh));
        }
        agents
    }

    pub(crate) fn receive_agent(&mut self, now: u32, agent: SimulationAgent) {
        let vehicle = self
            .departure_handler
            .handle_departure(now, agent, &mut self.garage);

        if let Some(vehicle) = vehicle {
            self.pass_vehicle_to_engine(now, vehicle, true);
        }
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

struct VehicularDepartureHandler {
    events: Rc<RefCell<EventsPublisher>>,
}

impl VehicularDepartureHandler {
    fn handle_departure(
        &mut self,
        now: u32,
        agent: SimulationAgent,
        garage: &mut Garage,
    ) -> Option<Vehicle> {
        assert_eq!(agent.state(), SimulationAgentState::LEG);

        let leg = agent.curr_leg();
        let route = leg.route.as_ref().unwrap();
        let leg_mode: Id<String> = Id::get(leg.mode);
        let veh_id = Id::get(route.veh_id);

        self.events.borrow_mut().publish_event(
            now,
            &Event::new_departure(agent.id(), route.start_link(), leg_mode.internal()),
        );

        let veh_type_id = garage
            .vehicles
            .get(&veh_id)
            .unwrap_or_else(|| panic!("Couldn't find vehicle with id {:?}", veh_id))
            .r#type;
        if LevelOfDetail::try_from(garage.vehicle_types.get(&Id::get(veh_type_id)).unwrap().lod)
            .unwrap()
            == LevelOfDetail::Network
        {
            self.events.borrow_mut().publish_event(
                now,
                &Event::new_person_enters_veh(agent.id(), veh_id.internal()),
            );
        }

        Some(garage.unpark_veh(agent, &veh_id))
    }
}
