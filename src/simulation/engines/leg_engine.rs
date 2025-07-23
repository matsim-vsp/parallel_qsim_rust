use crate::generated::events::Event;
use crate::simulation::agents::agent::SimulationAgent;
use crate::simulation::agents::{SimulationAgentLogic, SimulationAgentState};
use crate::simulation::config::Simulation;
use crate::simulation::controller::ThreadLocalComputationalEnvironment;
use crate::simulation::engines::network_engine::NetworkEngine;
use crate::simulation::engines::teleportation_engine::TeleportationEngine;
use crate::simulation::id::Id;
use crate::simulation::messaging::sim_communication::message_broker::NetMessageBroker;
use crate::simulation::messaging::sim_communication::SimCommunicator;
use crate::simulation::network::sim_network::SimNetworkPartition;
use crate::simulation::population::InternalRoute;
use crate::simulation::time_queue::Identifiable;
use crate::simulation::vehicles::garage::Garage;
use crate::simulation::vehicles::InternalVehicle;
use nohash_hasher::IntSet;

pub struct LegEngine<C: SimCommunicator> {
    teleportation_engine: TeleportationEngine,
    network_engine: NetworkEngine,
    garage: Garage,
    net_message_broker: NetMessageBroker<C>,
    departure_handler: VehicularDepartureHandler,
    main_modes: IntSet<Id<String>>,
    comp_env: ThreadLocalComputationalEnvironment,
}

impl<C: SimCommunicator> LegEngine<C> {
    pub fn new(
        network: SimNetworkPartition,
        garage: Garage,
        net_message_broker: NetMessageBroker<C>,
        config: &Simulation,
        comp_env: ThreadLocalComputationalEnvironment,
    ) -> Self {
        let main_modes: IntSet<Id<String>> = config
            .main_modes
            .iter()
            .map(|m| Id::<String>::get_from_ext(m))
            .collect();

        let departure_handler = VehicularDepartureHandler {
            comp_env: comp_env.clone(),
            main_modes: main_modes.clone(),
        };

        LegEngine {
            teleportation_engine: TeleportationEngine::new(comp_env.clone()),
            network_engine: NetworkEngine::new(network, comp_env.clone()),
            garage,
            net_message_broker,
            departure_handler,
            main_modes,
            comp_env,
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

        for mut msg in sync_messages {
            self.network_engine
                .network
                .apply_storage_cap_updates(msg.take_storage_capacities());

            for veh in msg.take_vehicles() {
                self.pass_vehicle_to_engine(now, veh, false);
            }
        }

        agents
    }

    fn publish_end_events(
        &mut self,
        now: u32,
        vehicles: Vec<InternalVehicle>,
        publish_leave_vehicle: bool,
    ) -> Vec<SimulationAgent> {
        let mut agents = vec![];
        for veh in vehicles {
            //in case of teleportation, do not publish leave vehicle events
            if publish_leave_vehicle {
                self.comp_env.events_publisher_borrow_mut().publish_event(
                    now,
                    &Event::new_person_leaves_veh(veh.driver().id().internal(), veh.id.internal()),
                );
            }

            for passenger in veh.passengers() {
                self.comp_env.events_publisher_borrow_mut().publish_event(
                    now,
                    &Event::new_passenger_dropped_off(
                        passenger.id().internal(),
                        passenger.curr_leg().mode.internal(),
                        0, //TODO
                        veh.id.internal(),
                    ),
                );
                if publish_leave_vehicle {
                    self.comp_env.events_publisher_borrow_mut().publish_event(
                        now,
                        &Event::new_person_leaves_veh(passenger.id().internal(), veh.id.internal()),
                    );
                }
            }

            let leg = veh.driver().curr_leg();
            self.comp_env.events_publisher_borrow_mut().publish_event(
                now,
                &Event::new_arrival(
                    veh.driver().id().internal(),
                    veh.curr_link_id().unwrap().internal(),
                    leg.mode.internal(),
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

    fn pass_vehicle_to_engine(&mut self, now: u32, vehicle: InternalVehicle, route_begin: bool) {
        let leg = vehicle.driver().curr_leg();

        // If mode of leg is not main mode, teleport vehicle in every case
        if !self.main_modes.contains(&leg.mode) {
            self.teleportation_engine
                .receive_vehicle(now, vehicle, &mut self.net_message_broker);
            return;
        }

        // Otherwise, make the decision based on the route type
        match leg.route.as_ref().unwrap() {
            InternalRoute::Network(_) => {
                self.network_engine
                    .receive_vehicle(now, vehicle, route_begin);
            }
            _ => {
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
    comp_env: ThreadLocalComputationalEnvironment,
    main_modes: IntSet<Id<String>>,
}

impl VehicularDepartureHandler {
    fn handle_departure(
        &mut self,
        now: u32,
        agent: SimulationAgent,
        garage: &mut Garage,
    ) -> Option<InternalVehicle> {
        assert_eq!(agent.state(), SimulationAgentState::LEG);

        let leg = agent.curr_leg();
        let route = leg
            .route
            .as_ref()
            .unwrap_or_else(|| panic!("Missing route for agent {} at leg {:?}", agent.id(), leg));

        self.comp_env.events_publisher_borrow_mut().publish_event(
            now,
            &Event::new_departure(
                agent.id().internal(),
                route.start_link().internal(),
                leg.mode.internal(),
            ),
        );

        let veh_id = route
            .as_generic()
            .vehicle()
            .as_ref()
            .unwrap_or(&Id::get_from_ext(&format!(
                "{}_{}",
                agent.id().external(),
                leg.mode.external()
            )))
            .clone();

        if self.main_modes.contains(&leg.mode) {
            assert!(
                route.as_network().is_some(),
                "{} is set as main mode but route is not network route",
                leg.mode
            );
            self.comp_env.events_publisher_borrow_mut().publish_event(
                now,
                &Event::new_person_enters_veh(agent.id().internal(), veh_id.internal()),
            );
        }

        Some(garage.unpark_veh(agent, veh_id))
    }
}
