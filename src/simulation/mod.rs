use crate::simulation::config::Config;
use crate::simulation::messaging::events::proto::Event;
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::messaging::message_broker::{MessageBroker, MpiMessageBroker};
use crate::simulation::messaging::messages::proto::leg::Route;
use crate::simulation::messaging::messages::proto::{
    Activity, Agent, GenericRoute, Vehicle, VehicleType,
};
use crate::simulation::network::link::Link;
use crate::simulation::network::network_partition::NetworkPartition;
use crate::simulation::network::node::{ExitReason, NodeVehicle};
use crate::simulation::network::routing_kit_network::RoutingKitNetwork;
use crate::simulation::population::Population;
use crate::simulation::routing::router::Router;
use crate::simulation::time_queue::TimeQueue;
use log::info;
use rust_road_router::algo::customizable_contraction_hierarchy::CCH;

pub mod config;
pub mod controller;
mod id_mapping;
pub mod io;
pub mod logging;
pub mod messaging;
#[allow(dead_code)]
mod network;
#[allow(dead_code)]
mod partition_info;
mod population;
#[allow(unused_comparisons, dead_code)]
mod routing;
mod simulation;
pub mod time_queue;

pub struct Simulation<'sim> {
    activity_q: TimeQueue<Agent>,
    teleportation_q: TimeQueue<Vehicle>,
    network: NetworkPartition<Vehicle>,
    message_broker: MpiMessageBroker,
    events: EventsPublisher,
    router: Option<Router<'sim>>,
    cch: &'sim Option<CCH>,
}

impl<'sim> Simulation<'sim> {
    pub fn new(
        config: &Config,
        network: NetworkPartition<Vehicle>,
        population: Population,
        message_broker: MpiMessageBroker,
        events: EventsPublisher,
        router: Option<Router<'sim>>,
        cch: &'sim Option<CCH>,
    ) -> Self {
        let mut activity_q = TimeQueue::new();
        for agent in population.agents.into_values() {
            activity_q.add(agent, config.start_time);
        }

        Simulation {
            network,
            teleportation_q: TimeQueue::new(),
            activity_q,
            message_broker,
            events,
            router,
            cch,
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

        for mut agent in agents {
            let agent_id = agent.id;
            self.events.publish_event(
                now,
                &Event::new_act_end(
                    agent_id,
                    agent.curr_act().link_id,
                    agent.curr_act().act_type.clone(),
                ),
            );

            if self.router.is_some() {
                let curr_act = agent.curr_act();
                let next_act = agent.next_act();

                let (route, travel_time) = self.find_route(curr_act, next_act);
                let dep_time = curr_act.end_time;

                agent.push_leg(dep_time, travel_time, route);
            }

            //here, current element counter is going to be increased
            agent.advance_plan();

            assert_ne!(agent.curr_plan_elem % 2, 0);

            let leg = agent.curr_leg();

            match leg.route.as_ref().unwrap() {
                Route::GenericRoute(route) => {
                    self.events.publish_event(
                        now,
                        &Event::new_departure(agent_id, route.start_link, leg.mode.clone()),
                    );

                    if Simulation::is_local_route(route, &self.message_broker) {
                        let veh = Vehicle::new(agent.id, VehicleType::Teleported, agent);
                        self.teleportation_q.add(veh, now);
                    } else {
                        let veh = Vehicle::new(agent.id, VehicleType::Teleported, agent);
                        self.message_broker.add_veh(veh, now);
                    }
                }
                Route::NetworkRoute(route) => {
                    let link_id = route.route.get(0).unwrap();
                    self.events.publish_event(
                        now,
                        &Event::new_departure(agent_id, *link_id, leg.mode.clone()),
                    );

                    self.events.publish_event(
                        now,
                        &Event::new_person_enters_veh(agent_id, route.vehicle_id),
                    );

                    let veh = Vehicle::new(route.vehicle_id, VehicleType::Network, agent);
                    self.veh_onto_network(veh, true, now);
                }
            }
        }
    }

    fn find_route(&mut self, from_act: &Activity, to_act: &Activity) -> (Vec<u64>, Option<u32>) {
        let query_result = self
            .router
            .as_mut()
            .unwrap()
            .query_coordinates(from_act.x, from_act.y, to_act.x, to_act.y);

        let route = query_result.path.expect("There is no route!");
        let trav_time = query_result.travel_time;
        (route, trav_time)
    }

    fn veh_onto_network(&mut self, vehicle: Vehicle, from_act: bool, now: u32) {
        let link_id = vehicle.curr_link_id().unwrap(); // in this case there should always be a link id.
        let link = self.network.links.get_mut(&link_id).unwrap();

        if !from_act {
            self.events
                .publish_event(now, &Event::new_link_enter(link_id as u64, vehicle.id));
        }
        match link {
            Link::LocalLink(link) => link.push_vehicle(vehicle, now),
            Link::SplitInLink(in_link) => {
                let local_link = in_link.local_link_mut();
                local_link.push_vehicle(vehicle, now)
            }
            Link::SplitOutLink(_) => {
                panic!("Vehicles should not start on out links...")
            }
        }
    }

    fn terminate_teleportation(&mut self, now: u32) {
        let teleportation_vehicles = self.teleportation_q.pop(now);
        for vehicle in teleportation_vehicles {
            // handle travelled
            let mut agent = vehicle.agent.unwrap();
            let leg = agent.curr_leg();
            if let Route::GenericRoute(route) = &leg.route.as_ref().unwrap() {
                self.events.publish_event(
                    now,
                    &Event::new_travelled(agent.id, route.distance, leg.mode.clone()),
                );
            }
            agent.advance_plan();
            self.activity_q.add(agent, now);
        }
    }

    fn move_nodes(&mut self, now: u32) {
        for node in self.network.nodes.values() {
            let exited_vehicles =
                node.move_vehicles(&mut self.network.links, now, &mut self.events);

            for exit_reason in exited_vehicles {
                match exit_reason {
                    ExitReason::FinishRoute(vehicle) => {
                        let veh_id = vehicle.id;
                        let mut agent = vehicle.agent.unwrap();
                        let leg_mode = agent.curr_leg().mode.clone();

                        self.events
                            .publish_event(now, &Event::new_person_leaves_veh(agent.id, veh_id));

                        agent.advance_plan();
                        let act = agent.curr_act();

                        self.events.publish_event(
                            now,
                            &Event::new_arrival(agent.id, act.link_id, leg_mode),
                        );

                        self.events.publish_event(
                            now,
                            &Event::new_act_start(agent.id, act.link_id, act.act_type.clone()),
                        );
                        self.activity_q.add(agent, now);
                    }
                    ExitReason::ReachedBoundary(vehicle) => {
                        self.message_broker.add_veh(vehicle, now);
                    }
                }
            }
        }
    }

    fn send_receive(&mut self, now: u32) {
        self.events
            .get_travel_time_collector()
            .unwrap()
            .get_travel_time_of_link(1);
        //send mechanism
        //receive mechanism
        let vehicles = self.message_broker.send_recv(now);
        for vehicle in vehicles {
            match vehicle.r#type() {
                VehicleType::Teleported => {
                    self.teleportation_q.add(vehicle, now);
                }
                VehicleType::Network => {
                    self.veh_onto_network(vehicle, false, now);
                }
            }
        }

        //TODO
        if let Some(router) = self.router.as_mut() {
            router.customize(self.cch.as_ref().unwrap(), RoutingKitNetwork::new());
        }
    }

    fn is_local_route(route: &GenericRoute, message_broker: &MpiMessageBroker) -> bool {
        let (from, to) = Simulation::process_ids_for_generic_route(route, message_broker);
        from == to
    }
    fn process_ids_for_generic_route(
        route: &GenericRoute,
        message_broker: &MpiMessageBroker,
    ) -> (u64, u64) {
        let from_rank = message_broker.rank_for_link(route.start_link);
        let to_rank = message_broker.rank_for_link(route.end_link);
        (from_rank, to_rank)
    }
}
