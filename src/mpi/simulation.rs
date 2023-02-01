use crate::config::Config;
use crate::mpi::events::proto::Event;
use crate::mpi::events::EventsPublisher;
use crate::mpi::message_broker::{MessageBroker, MpiMessageBroker};
use crate::mpi::messages::proto::leg::Route;
use crate::mpi::messages::proto::{Agent, GenericRoute, Leg, NetworkRoute, Vehicle, VehicleType};
use crate::mpi::population::Population;
use crate::mpi::time_queue::TimeQueue;
use crate::parallel_simulation::network::link::Link;
use crate::parallel_simulation::network::network_partition::NetworkPartition;
use crate::parallel_simulation::network::node::{ExitReason, NodeVehicle};
use crate::parallel_simulation::routing::router::Router;
use log::info;

pub struct Simulation<'sim> {
    activity_q: TimeQueue<Agent>,
    teleportation_q: TimeQueue<Vehicle>,
    network: NetworkPartition<Vehicle>,
    message_broker: MpiMessageBroker,
    events: EventsPublisher,
    router: Option<Router<'sim>>,
}

impl<'sim> Simulation<'sim> {
    pub fn new(
        config: &Config,
        network: NetworkPartition<Vehicle>,
        population: Population,
        message_broker: MpiMessageBroker,
        events: EventsPublisher,
        router: Option<Router<'sim>>,
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
        let mut agents = self.activity_q.pop(now);

        for agent in &mut agents {
            let agent_id = agent.id;
            self.events.publish_event(
                now,
                &Event::new_act_end(
                    agent_id,
                    agent.curr_act().link_id,
                    agent.curr_act().act_type.clone(),
                ),
            );

            if let Some(router) = self.router.as_mut() {
                let dep_time;
                let trav_time;
                let route;
                {
                    let end_activity = agent.curr_act();

                    let new_activity = agent.next_act();

                    let query_result = router.query_coordinates(
                        end_activity.x,
                        end_activity.y,
                        new_activity.x,
                        new_activity.y,
                    );

                    dep_time = end_activity.end_time;
                    trav_time = query_result.travel_time;
                    route = query_result.path.expect("There is no route!");
                }

                agent.plan.as_mut().unwrap().legs.push(Leg {
                    mode: "car".to_string(),
                    dep_time,
                    trav_time,
                    route: Some(Route::NetworkRoute(NetworkRoute {
                        vehicle_id: 0, //TODO
                        route,
                    })),
                });
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
                        let veh = Vehicle::new(agent.id, VehicleType::Teleported, agent.clone());
                        self.teleportation_q.add(veh, now);
                    } else {
                        let veh = Vehicle::new(agent.id, VehicleType::Teleported, agent.clone());
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

                    let veh = Vehicle::new(route.vehicle_id, VehicleType::Network, agent.clone());
                    self.veh_onto_network(veh, true, now);
                }
            }
        }
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
