use crate::simulation::engines::network_engine::NetworkEngine;
use crate::simulation::engines::teleportation_engine::TeleportationEngine;
use crate::simulation::engines::{AgentStateTransitionLogic, Engine};
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
use std::cell::RefCell;
use std::rc::{Rc, Weak};

pub struct LegEngine<C: SimCommunicator> {
    teleportation_engine: TeleportationEngine,
    network_engine: NetworkEngine,
    garage: Garage,
    net_message_broker: NetMessageBroker<C>,
    events: Rc<RefCell<EventsPublisher>>,
    agent_state_transition_logic: Weak<RefCell<AgentStateTransitionLogic>>,
}

impl<C: SimCommunicator + 'static> Engine for LegEngine<C> {
    fn do_step(&mut self, now: u32) {
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

    fn receive_agent(&mut self, now: u32, agent: Person) {
        let vehicle = self.place_agent_in_vehicle(now, agent);
        self.pass_vehicle_to_engine(now, vehicle, true);
    }

    fn set_agent_state_transition_logic(
        &mut self,
        agent_state_transition_logic: Weak<RefCell<AgentStateTransitionLogic>>,
    ) {
        self.agent_state_transition_logic = agent_state_transition_logic
    }
}

impl<C: SimCommunicator + 'static> LegEngine<C> {
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
        }
    }

    pub fn net_message_broker(&self) -> &NetMessageBroker<C> {
        &self.net_message_broker
    }

    pub fn network(&self) -> &SimNetworkPartition {
        &self.network_engine.network
    }

    fn place_agent_in_vehicle(&mut self, now: u32, agent: Person) -> Vehicle {
        assert_ne!(agent.curr_plan_elem % 2, 0);

        let leg = agent.curr_leg();
        let route = leg.route.as_ref().unwrap();
        let leg_mode: Id<String> = Id::get(leg.mode);
        self.events.borrow_mut().publish_event(
            now,
            &Event::new_departure(agent.id, route.start_link(), leg_mode.internal()),
        );

        let veh_id = Id::get(route.veh_id);
        self.garage.unpark_veh(agent, &veh_id)
    }
}
