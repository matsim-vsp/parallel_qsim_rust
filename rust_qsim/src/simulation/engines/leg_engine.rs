use crate::simulation::agents::agent::SimulationAgent;
use crate::simulation::agents::{SimulationAgentLogic, SimulationAgentState};
use crate::simulation::config::Simulation;
use crate::simulation::controller::ThreadLocalComputationalEnvironment;
use crate::simulation::engines::network_engine::NetworkEngine;
use crate::simulation::engines::teleportation_engine::TeleportationEngine;
use crate::simulation::events::{
    PersonArrivalEventBuilder, PersonDepartureEventBuilder, PersonEntersVehicleEventBuilder,
    PersonLeavesVehicleEventBuilder,
};
use crate::simulation::id::Id;
use crate::simulation::messaging::sim_communication::message_broker::NetMessageBroker;
use crate::simulation::messaging::sim_communication::SimCommunicator;
use crate::simulation::network::sim_network::SimNetworkPartition;
use crate::simulation::population::InternalRoute;
use crate::simulation::time_queue::Identifiable;
use crate::simulation::vehicles::garage::Garage;
use crate::simulation::vehicles::InternalVehicle;
use nohash_hasher::IntSet;
use tracing::instrument;
use crate::simulation::messaging::messages::InternalSyncMessage;

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

    /// Performs a sim step for the leg engine. Note that vehicles that leave a link and move to another link are always processed one time step later.
    /// This is in line with the Java reference implementation. The reason is that the order is:
    ///
    /// 1. `move_nodes`
    /// 2. `move_links`
    ///
    /// Let's say, a vehicle's earliest exit time is `x`. The `move_links` call puts it into the buffer
    /// at time step `x` (assuming it is free), and the `move_nodes` call at time step `x+1` puts it onto the next link.
    /// The corresponding LinkEnter and LinkLeave events have time step `x+1`
    #[instrument(level = "trace", skip(self, agents), fields(rank=self.net_message_broker.rank()))]
    pub(crate) fn do_step(
        &mut self,
        now: u32,
        agents: Vec<SimulationAgent>,
    ) -> Vec<SimulationAgent> {
        self.receive_agents(now, agents);

        let teleported_vehicles = self.teleportation_engine.do_step(now);

        self.network_engine.move_nodes(now);
        let network_vehicles = self
            .network_engine
            .move_links(now, &mut self.net_message_broker);

        let sync_messages = self.send_recv(now);

        for mut msg in sync_messages {
            self.network_engine
                .network
                .apply_storage_cap_updates(msg.take_storage_capacities());

            for veh in msg.take_vehicles() {
                self.pass_vehicle_to_engine(now, veh, false);
            }
        }

        let mut agents = vec![];
        agents.extend(self.publish_end_events(now, network_vehicles, true));
        agents.extend(self.publish_end_events(now, teleported_vehicles, false));
        agents
    }

    #[instrument(level = "trace", skip(self), fields(rank=self.net_message_broker.rank()))]
    fn send_recv(&mut self, now: u32) -> Vec<InternalSyncMessage> {
        let sync_messages = self.net_message_broker.send_recv(now);
        sync_messages
    }

    fn receive_agents(&mut self, now: u32, agents: Vec<SimulationAgent>) {
        for agent in agents {
            self.receive_agent(now, agent);
        }
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
                    &PersonLeavesVehicleEventBuilder::default()
                        .time(now)
                        .vehicle(veh.id.clone())
                        .person(veh.driver().id().clone())
                        .build()
                        .unwrap(),
                );
            }

            for passenger in veh.passengers() {
                if publish_leave_vehicle {
                    self.comp_env.events_publisher_borrow_mut().publish_event(
                        &PersonLeavesVehicleEventBuilder::default()
                            .time(now)
                            .vehicle(veh.id.clone())
                            .person(passenger.id().clone())
                            .build()
                            .unwrap(),
                    );
                }
            }

            let leg = veh.driver().curr_leg();
            self.comp_env.events_publisher_borrow_mut().publish_event(
                &PersonArrivalEventBuilder::default()
                    .time(now)
                    .person(veh.driver().id().clone())
                    .link(veh.curr_link_id().unwrap().clone())
                    .leg_mode(leg.mode.clone())
                    .build()
                    .unwrap(),
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
            &PersonDepartureEventBuilder::default()
                .time(now)
                .person(agent.id().clone())
                .link(route.start_link().clone())
                .leg_mode(leg.mode.clone())
                .build()
                .unwrap(),
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
                &PersonEntersVehicleEventBuilder::default()
                    .time(now)
                    .person(agent.id().clone())
                    .vehicle(veh_id.clone())
                    .build()
                    .unwrap(),
            );
        }

        Some(garage.unpark_veh(agent, veh_id))
    }
}
