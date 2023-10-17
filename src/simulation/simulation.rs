use std::sync::Arc;

use tracing::info;

use crate::simulation::config::Config;
use crate::simulation::messaging::events::proto::Event;
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::messaging::message_broker::{NetCommunicator, NetMessageBroker};
use crate::simulation::messaging::messages::proto::{Agent, Vehicle};
use crate::simulation::network::sim_network::{ExitReason, SimNetworkPartition};
use crate::simulation::population::population::Population;
use crate::simulation::time_queue::TimeQueue;
use crate::simulation::vehicles::garage::Garage;
use crate::simulation::vehicles::vehicle_type::LevelOfDetail;

pub struct Simulation<'sim, C>
where
    C: NetCommunicator,
{
    activity_q: TimeQueue<Agent>,
    teleportation_q: TimeQueue<Vehicle>,
    network: SimNetworkPartition<'sim>,
    population: Population<'sim>,
    garage: Garage<'sim>,
    message_broker: NetMessageBroker<C>,
    events: EventsPublisher,
}

impl<'sim, C> Simulation<'sim, C>
where
    C: NetCommunicator,
{
    pub fn new(
        config: Arc<Config>,
        network: SimNetworkPartition<'sim>,
        garage: Garage<'sim>,
        mut population: Population<'sim>,
        message_broker: NetMessageBroker<C>,
        events: EventsPublisher,
    ) -> Self {
        let mut activity_q = TimeQueue::new();

        // take agents and copy them into queues. This way we can keep population around to translate
        // ids for events processing...
        let agents = std::mem::take(&mut population.agents);

        for agent in agents.into_values() {
            activity_q.add(agent, config.start_time);
        }

        Simulation {
            network,
            population,
            garage,
            teleportation_q: TimeQueue::new(),
            activity_q,
            message_broker,
            events,
        }
    }

    pub fn run(&mut self, start_time: u32, end_time: u32) {
        // use fixed start and end times
        let mut now = start_time;
        info!(
            "Starting #{}. Network neighbors: {:?}, Start time {start_time}, End time {end_time}",
            self.message_broker.rank,
            self.network.neighbors(),
        );

        while now <= end_time {
            if self.message_broker.rank == 0 && now % 600 == 0 {
                let _hour = now / 3600;
                let _min = (now % 3600) / 60;
                info!("#{} of Qsim at {_hour}:{_min}", self.message_broker.rank);
            }
            self.wakeup(now);
            self.terminate_teleportation(now);
            self.move_nodes(now);
            self.send_receive(now);

            now += 1;
        }

        // maybe this belongs into the controller? Then this would have to be a &mut instead of owned.
        self.events.finish();
    }

    fn wakeup(&mut self, now: u32) {
        let agents = self.activity_q.pop(now);

        for agent in agents {
            let act_type = self
                .population
                .act_types
                .get_from_wire(agent.curr_act().act_type);
            self.events.publish_event(
                now,
                &Event::new_act_end(
                    agent.id,
                    agent.curr_act().link_id,
                    act_type.external().to_string(),
                ),
            );

            let mut vehicle = self.departure(agent, now);
            let veh_type_id = self.garage.vehicle_type_ids.get_from_wire(vehicle.r#type);
            let veh_type = self.garage.vehicle_types.get(&veh_type_id).unwrap();

            match veh_type.lod {
                LevelOfDetail::Network => {
                    self.events.publish_event(
                        now,
                        &Event::new_person_enters_veh(vehicle.agent().id, vehicle.id),
                    );
                    self.network.send_veh_en_route(vehicle, now);
                }
                LevelOfDetail::Teleported => {
                    if Simulation::is_local_route(&vehicle, &self.message_broker) {
                        self.teleportation_q.add(vehicle, now);
                    } else {
                        // we need to call advance here, so that the vehicle's current link index
                        // points to the end link of the route array.
                        vehicle.advance_route_index();
                        self.message_broker.add_veh(vehicle, now);
                    }
                }
            }
        }
    }

    fn departure(&mut self, mut agent: Agent, now: u32) -> Vehicle {
        //here, current element counter is going to be increased
        agent.advance_plan();

        assert_ne!(agent.curr_plan_elem % 2, 0);

        let leg = agent.curr_leg();
        let route = leg.route.as_ref().unwrap();
        let leg_mode = self.garage.modes.get_from_wire(leg.mode);
        self.events.publish_event(
            now,
            &Event::new_departure(
                agent.id,
                route.start_link(),
                leg_mode.external().to_string(),
            ),
        );

        let veh_id = self.garage.vehicle_ids.get_from_wire(route.veh_id);
        self.garage.unpark_veh(agent, &veh_id)
    }

    fn terminate_teleportation(&mut self, now: u32) {
        let teleportation_vehicles = self.teleportation_q.pop(now);
        for vehicle in teleportation_vehicles {
            // park the vehice - get the agent out of the vehicle
            let mut agent = self.garage.park_veh(vehicle);

            // handle travelled
            let leg = agent.curr_leg();
            let route = leg.route.as_ref().unwrap();
            let mode = self.garage.modes.get_from_wire(leg.mode);
            self.events.publish_event(
                now,
                &Event::new_travelled(agent.id, route.distance, mode.external().to_string()),
            );

            // advance plan to activity and put agent into activity q.
            agent.advance_plan();

            // handle act start event
            let act = agent.curr_act();
            let act_type = self.population.act_types.get_from_wire(act.act_type);
            self.events.publish_event(
                now,
                &Event::new_act_start(agent.id, act.link_id, act_type.external().to_string()),
            );
            self.activity_q.add(agent, now);
        }
    }

    fn move_nodes(&mut self, now: u32) {
        let exited_vehicles = self.network.move_nodes(&mut self.events, now);
        self.network.move_links();

        for exit_reason in exited_vehicles {
            match exit_reason {
                ExitReason::FinishRoute(vehicle) => {
                    // finish driving - get agent out of vehicle, remember mode for arrival event
                    self.events.publish_event(
                        now,
                        &Event::new_person_leaves_veh(vehicle.agent().id, vehicle.id),
                    );
                    let veh_type_id = self.garage.vehicle_type_ids.get_from_wire(vehicle.r#type);
                    let veh_type = self.garage.vehicle_types.get(&veh_type_id).unwrap();
                    let mode = veh_type.net_mode.external().to_string();
                    let mut agent = self.garage.park_veh(vehicle);

                    // move to next activity
                    agent.advance_plan();
                    let act = agent.curr_act();
                    self.events
                        .publish_event(now, &Event::new_arrival(agent.id, act.link_id, mode));
                    let act_type = self.population.act_types.get_from_wire(act.act_type);
                    self.events.publish_event(
                        now,
                        &Event::new_act_start(
                            agent.id,
                            act.link_id,
                            act_type.external().to_string(),
                        ),
                    );
                    self.activity_q.add(agent, now);
                }
                ExitReason::ReachedBoundary(vehicle) => {
                    self.message_broker.add_veh(vehicle, now);
                }
            }
        }
    }

    fn send_receive(&mut self, now: u32) {
        let vehicle_update_messages = self.message_broker.send_recv(now);

        let vehicles = vehicle_update_messages
            .into_iter()
            .flat_map(|msg| msg.vehicles)
            .collect::<Vec<Vehicle>>();

        for vehicle in vehicles {
            match vehicle.r#type {
                1 => {
                    self.teleportation_q.add(vehicle, now);
                }
                0 => {
                    self.events.publish_event(
                        now,
                        &Event::new_link_enter(vehicle.curr_link_id().unwrap() as u64, vehicle.id),
                    );
                    self.network.send_veh_en_route(vehicle, now);
                }
                _ => {}
            }
        }
    }

    fn is_local_route(veh: &Vehicle, message_broker: &NetMessageBroker<C>) -> bool {
        let leg = veh.agent.as_ref().unwrap().curr_leg();
        let route = leg.route.as_ref().unwrap();
        let from = message_broker.rank_for_link(route.start_link());
        let to = message_broker.rank_for_link(route.end_link());
        from == to
    }
}
